#![allow(dead_code)]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    common::{
        camera::{Camera, CameraError, CameraType},
        config::ConfigType,
        registry::{ComponentRegistry, Dependency},
        status::{Status, StatusError},
    },
    esp32::esp_idf_svc::sys::camera::{
        camera_config_t, camera_config_t__bindgen_ty_1, camera_config_t__bindgen_ty_2, camera_fb_t,
        esp_camera_fb_get, esp_camera_fb_return, esp_camera_init,
    },
    google::{self, api::HttpBody},
    proto::component::camera,
    systime::EspSystemTime,
};
use bytes::{Bytes, BytesMut};
use log::*;
use prost::Message;

#[derive(DoCommand)]
pub struct Esp32Camera {
    config: camera_config_t,
    last_grab: Duration,
}

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_camera("esp32camera", &Esp32Camera::from_config)
        .is_err()
    {
        log::error!("esp32camera type is already registered");
    }
}

impl Esp32Camera {
    pub(crate) fn from_config(
        _cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<CameraType, CameraError> {
        let cam = Self::new();
        cam.setup()?;
        Ok(Arc::new(Mutex::new(cam)))
    }

    pub fn new() -> Self {
        let t = EspSystemTime;
        Esp32Camera {
            config: camera_config_t {
                pin_pwdn: -1,
                pin_reset: -1,
                pin_xclk: 21,
                __bindgen_anon_1: camera_config_t__bindgen_ty_1 { pin_sccb_sda: 26 },
                __bindgen_anon_2: camera_config_t__bindgen_ty_2 { pin_sccb_scl: 27 },
                pin_d7: 35,
                pin_d6: 34,
                pin_d5: 39,
                pin_d4: 36,
                pin_d3: 19,
                pin_d2: 18,
                pin_d1: 5,
                pin_d0: 4,
                pin_vsync: 25,
                pin_href: 23,
                pin_pclk: 22,
                xclk_freq_hz: 20000000,
                ledc_timer: 1,
                ledc_channel: 1,
                pixel_format: 4,
                frame_size: 4,
                jpeg_quality: 32,
                fb_count: 1,
                grab_mode: 0,
                fb_location: 0,
                sccb_i2c_port: 0,
            },
            last_grab: t.now(),
        }
    }
    fn setup(&self) -> Result<(), CameraError> {
        let ret =
            (unsafe { esp_camera_init(&self.config) }) as crate::esp32::esp_idf_svc::sys::esp_err_t;
        let ret = crate::esp32::esp_idf_svc::sys::EspError::convert(ret);
        ret.map_err(|e| CameraError::InitError(e.into()))
    }
    pub fn get_cam_frame(&self) -> Option<*mut camera_fb_t> {
        let ptr = (unsafe { esp_camera_fb_get() }) as *mut camera_fb_t;
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    pub fn return_cam_frame(&self, frame: Option<*mut camera_fb_t>) {
        if let Some(ptr) = frame {
            unsafe { esp_camera_fb_return(ptr) }
        }
    }
    pub fn debug_print_fb(&self, frame: &Option<*mut camera_fb_t>) {
        if let Some(ptr) = frame {
            let ptr = ptr as &*mut camera_fb_t;
            unsafe {
                println!();
                info!("camera buf size {}", (*(*ptr)).len);
                println!();
                for i in 0..(*(*ptr)).len {
                    print!("{:02X}", (*(*(*ptr)).buf.offset(i as isize)));
                    if i > 0 && i % 80 == 0 {
                        println!();
                    }
                }
            }
        }
    }
}
impl Camera for Esp32Camera {
    fn get_image(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        if let Some(ptr) = self.get_cam_frame() {
            let buf = unsafe {
                let buf = (*ptr).buf;
                let len = (*ptr).len as usize;
                core::slice::from_raw_parts(buf, len)
            };
            if buf.len() > buffer.capacity() {
                self.return_cam_frame(Some(ptr));
                return Err(CameraError::ImageTooBig);
            }
            let bytes = Bytes::from(buf);
            let msg = camera::v1::GetImageResponse {
                mime_type: "image/jpeg".to_string(),
                image: bytes,
            };
            msg.encode(&mut buffer).unwrap();
            self.return_cam_frame(Some(ptr));
            return Ok(buffer);
        }
        Err(CameraError::FailedToGetImage)
    }

    fn render_frame(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        if let Some(ptr) = self.get_cam_frame() {
            let buf = unsafe {
                let buf = (*ptr).buf;
                let len = (*ptr).len as usize;
                core::slice::from_raw_parts(buf, len)
            };
            if buf.len() > buffer.capacity() {
                self.return_cam_frame(Some(ptr));
                return Err(CameraError::ImageTooBig);
            }
            let bytes = Bytes::from(buf);
            let msg = HttpBody {
                content_type: "image/jpeg".to_string(),
                data: bytes.to_vec(),
                ..Default::default()
            };
            msg.encode(&mut buffer).unwrap();
            self.return_cam_frame(Some(ptr));
            return Ok(buffer);
        }
        Err(CameraError::FailedToGetImage)
    }
}

impl Status for Esp32Camera {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
