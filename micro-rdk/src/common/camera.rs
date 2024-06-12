use super::{
    generic::DoCommand,
    status::{Status, StatusError},
};
use crate::{google, proto::component::camera};
use bytes::BytesMut;
use prost::Message;
use std::{collections::HashMap, sync::Mutex};
use thiserror::Error;

#[cfg(feature = "camera")]
use {
    super::{config::ConfigType, registry::ComponentRegistry, registry::Dependency},
    std::sync::Arc,
};

pub static COMPONENT_NAME: &str = "camera";
#[cfg(feature = "camera")]
pub static FAKE_JPEG: &[u8] = include_bytes!("../../../etc/assets/symbol.jpg");

#[derive(Error, Debug)]
pub enum CameraError {
    #[error("cannot build camera {0}")]
    CameraInitError(#[from] Box<dyn std::error::Error + Sync + Send>),
    #[error("frame too big for buffer")]
    CameraFrameTooBig,
    #[error("couldn't get frame")]
    CameraCouldntGetFrame,
}

#[cfg(feature = "camera")]
pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_camera("fake", &FakeCamera::from_config)
        .is_err()
    {
        log::error!("fake camera type is already registered");
    }
}

pub trait Camera: Status + DoCommand {
    fn get_image(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError>;
    fn get_images(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError>;
    fn get_point_cloud(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError>;
    fn get_properties(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError>;
    fn render_frame(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError>;
}

#[cfg(feature = "camera")]
pub(crate) type CameraType = Arc<Mutex<dyn Camera>>;

#[derive(DoCommand)]
pub struct FakeCamera {}

impl FakeCamera {
    pub fn new() -> Self {
        FakeCamera {}
    }
    #[cfg(feature = "camera")]
    pub(crate) fn from_config(
        _cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<CameraType, CameraError> {
        Ok(Arc::new(Mutex::new(FakeCamera::new())))
    }
}

impl Default for FakeCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl Camera for FakeCamera {
    fn get_image(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        #[cfg(feature = "camera")]
        let image = FAKE_JPEG;
        #[cfg(not(feature = "camera"))]
        let image = BytesMut::new();
        let msg = camera::v1::GetImageResponse {
            mime_type: "image/jpeg".to_string(),
            image: image.into(),
        };
        msg.encode(&mut buffer).unwrap();
        Ok(buffer)
    }
    fn get_images(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        unimplemented!();
    }
    fn get_point_cloud(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        unimplemented!();
    }
    fn get_properties(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        unimplemented!();
    }
    fn render_frame(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        #[cfg(feature = "camera")]
        let msg = google::api::HttpBody {
            content_type: "image/jpeg".to_string(),
            data: FAKE_JPEG.into(),
            ..Default::default()
        };
        #[cfg(not(feature = "camera"))]
        let msg = google::protobuf::Struct::default();
        msg.encode(&mut buffer).unwrap();
        Ok(buffer)
    }
}

impl<L> Camera for Mutex<L>
where
    L: ?Sized + Camera,
{
    fn get_image(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().get_image(buffer)
    }
    fn get_images(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().get_images(buffer)
    }
    fn get_point_cloud(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().get_point_cloud(buffer)
    }
    fn get_properties(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().get_properties(buffer)
    }
    fn render_frame(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().render_frame(buffer)
    }
}

impl Status for FakeCamera {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
