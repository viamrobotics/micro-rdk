#![allow(dead_code)]
use std::sync::{Arc, Mutex};

use crate::proto::component::camera;
use bytes::{Bytes, BytesMut};
use prost::Message;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CameraError {
    #[error("cannot build camera {0}")]
    CameraInitError(#[from] Box<dyn std::error::Error + Sync + Send>),
    #[error("frame too big for buffer")]
    CameraFrameTooBig,
    #[error("couldn't get frame")]
    CameraCouldntGetFrame,
}

pub static COMPONENT_NAME: &str = "camera";

pub trait Camera {
    fn get_frame(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError>;
}

pub(crate) type CameraType = Arc<Mutex<dyn Camera>>;

pub struct FakeCamera {}

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

impl FakeCamera {
    pub fn new() -> Self {
        FakeCamera {}
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
}
