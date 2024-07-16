use super::{generic::DoCommand, registry::ComponentRegistry, status::Status};
use bytes::BytesMut;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[cfg(feature = "builtin-components")]
mod fake_camera;

#[allow(unused)]
pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    #[cfg(feature = "camera")]
    {
        #[cfg(feature = "builtin-components")]
        {
            fake_camera::register_models(registry);
            #[cfg(feature = "esp32")]
            crate::esp32::camera::register_models(registry);
        }
    }
}

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
    /// Returns a structured image response from a camera of the underlying robot.
    /// A specific MIME type can be requested but may not necessarily be the same one returned
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

impl<L> Camera for Arc<Mutex<L>>
where
    L: ?Sized + Camera,
{
    fn get_image(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.lock().unwrap().get_image(buffer)
    }
    fn get_images(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.lock().unwrap().get_images(buffer)
    }
    fn get_point_cloud(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.lock().unwrap().get_point_cloud(buffer)
    }
    fn get_properties(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.lock().unwrap().get_properties(buffer)
    }
    fn render_frame(&mut self, buffer: BytesMut) -> Result<BytesMut, CameraError> {
        self.lock().unwrap().render_frame(buffer)
    }
}
