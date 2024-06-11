use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use super::generic::DoCommand;
use super::{
    config::ConfigType,
    registry::{ComponentRegistry, Dependency},
};
use crate::proto::component::camera;
use bytes::{Bytes, BytesMut};
use prost::Message;

use thiserror::Error;

pub static COMPONENT_NAME: &str = "camera";

#[derive(Error, Debug)]
pub enum CameraError {
    #[error("cannot build camera {0}")]
    CameraInitError(#[from] Box<dyn std::error::Error + Sync + Send>),
    #[error("frame too big for buffer")]
    CameraFrameTooBig,
    #[error("couldn't get frame")]
    CameraCouldntGetFrame,
}

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_camera("fake", &FakeCamera::from_config)
        .is_err()
    {
        log::error!("fake camera type is already registered");
    }
}

pub trait Camera {
    fn get_frame(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError>;
    fn get_frames(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        unimplemented!();
    }
    fn get_point_cloud(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        unimplemented!();
    }
    fn get_properties(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        unimplemented!();
    }
    fn render_frame(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        unimplemented!();
    }
    fn do_command(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        unimplemented!();
    }
}

pub(crate) type CameraType = Arc<Mutex<dyn Camera>>;

#[derive(DoCommand)]
pub struct FakeCamera {}

impl FakeCamera {
    pub fn new() -> Self {
        FakeCamera {}
    }
    pub(crate) fn from_config(
        _cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<CameraType, CameraError> {
        log::info!("making new camera");
        Ok(Arc::new(Mutex::new(FakeCamera::new())))
    }
}

impl Camera for FakeCamera {
    fn get_frame(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        let msg = camera::v1::GetImageResponse {
            mime_type: "image/jpeg".to_string(),
            image: Bytes::new(),
        };

        msg.encode(&mut buffer).unwrap();

        Ok(buffer)
    }
}

impl Default for FakeCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl<L> Camera for Mutex<L>
where
    L: ?Sized + Camera,
{
    fn get_frame(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().get_frame(buffer)
    }
    fn get_frames(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().get_frames(buffer)
    }
    fn get_point_cloud(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().get_point_cloud(buffer)
    }
    fn get_properties(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().get_properties(buffer)
    }
    fn do_command(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.get_mut().unwrap().do_command(buffer)
    }
}
