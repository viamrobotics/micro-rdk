use crate::{
    common::{
        camera::{Camera, CameraError, CameraType},
        status::{Status, StatusError},
    },
    google,
    proto::component::camera::v1::GetImageResponse,
};
use bytes::BytesMut;
use prost::Message;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::common::{config::ConfigType, registry::ComponentRegistry, registry::Dependency};

pub static FAKE_JPEG: &[u8] = include_bytes!("../../common/camera/fake_image.jpg");

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_camera("fake", &FakeCamera::from_config)
        .is_err()
    {
        log::error!("fake camera type is already registered");
    }
}

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
        let msg = GetImageResponse {
            mime_type: "image/jpeg".to_string(),
            image: FAKE_JPEG.into(),
        };
        msg.encode(&mut buffer)
            .map_err(|_| CameraError::CameraGenericError("failed to encode GetImageResponse"))?;
        Ok(buffer)
    }
    fn render_frame(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        let msg = google::api::HttpBody {
            content_type: "image/jpeg".to_string(),
            data: FAKE_JPEG.to_vec(),
            ..Default::default()
        };
        msg.encode(&mut buffer)
            .map_err(|_| CameraError::CameraGenericError("failed to encode RenderFrameResponse"))?;
        Ok(buffer)
    }
}

impl Status for FakeCamera {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
