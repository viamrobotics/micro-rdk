#![allow(dead_code)]
use std::sync::Mutex;

use crate::proto::component::camera;
use bytes::{Bytes, BytesMut};
use prost::Message;

pub trait Camera {
    fn get_frame(&mut self, buffer: BytesMut) -> anyhow::Result<BytesMut>;
}

pub struct FakeCamera {}

impl Camera for FakeCamera {
    fn get_frame(&mut self, mut buffer: BytesMut) -> anyhow::Result<BytesMut> {
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
    fn get_frame(&mut self, buffer: BytesMut) -> anyhow::Result<BytesMut> {
        self.get_mut().unwrap().get_frame(buffer)
    }
}
