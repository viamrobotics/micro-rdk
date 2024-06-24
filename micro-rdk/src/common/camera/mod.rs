use super::{generic::DoCommand, status::Status};
use bytes::BytesMut;
use std::sync::{Arc, Mutex};
use thiserror::Error;

// Enables FakeCamera for native server
#[cfg(all(feature = "camera", feature = "native", feature = "builtin-components"))]
mod fake_camera;
#[cfg(all(feature = "camera", feature = "native", feature = "builtin-components"))]
pub(crate) use fake_camera::register_models;

#[allow(dead_code)]
pub(crate) type CameraType = Arc<Mutex<dyn Camera>>;
pub static COMPONENT_NAME: &str = "camera";

#[derive(Error, Debug)]
pub enum CameraError {
    #[error("cannot build camera {0}")]
    InitError(#[from] Box<dyn std::error::Error + Sync + Send>),
    #[error("config error {0}")]
    ConfigError(&'static str),
    #[error("frame too big for buffer")]
    ImageTooBig,
    #[error("failed to get image")]
    FailedToGetImage,
    #[error("method {0} unimplemented")]
    CameraMethodUnimplemented(&'static str),
    #[error("{0}")]
    CameraGenericError(&'static str),
}

pub trait Camera: Status + DoCommand {
    /// returns an image from a camera of the underlying robot. A specific MIME type
    /// can be requested but may not necessarily be the same one returned
    fn get_image(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        Err(CameraError::CameraMethodUnimplemented("get_image"))
    }
    fn get_images(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        Err(CameraError::CameraMethodUnimplemented("get_images"))
    }
    fn get_point_cloud(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        Err(CameraError::CameraMethodUnimplemented("get_point_cloud"))
    }
    /// Returns the camera intrinsic parameters and camera distortion parameters
    fn get_properties(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        Err(CameraError::CameraMethodUnimplemented("get_properties"))
    }
    /// Deprecated, use `get_image` instead.
    fn render_frame(&mut self, _buffer: BytesMut) -> Result<BytesMut, CameraError> {
        Err(CameraError::CameraMethodUnimplemented("render_frame"))
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
