#![allow(dead_code)]
use std::time::Duration;

use crate::common::camera::{Camera, CameraError};
use crate::proto::component::camera;
use bytes::{Bytes, BytesMut};
use log::*;
use prost::Message;

/*
use crate::esp32::esp_idf_svc::sys::camera_config_t;
use crate::esp32::esp_idf_svc::sys::camera_config_t__bindgen_ty_1;
use crate::esp32::esp_idf_svc::sys::camera_config_t__bindgen_ty_2;
use crate::esp32::esp_idf_svc::systime::EspSystemTime;

pub struct Esp32Camera {
    config: camera_config_t,
    last_grab: Duration,
}

impl Esp32Camera {
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
    pub fn setup(&self) -> Result<(), CameraError> {
        let ret = (unsafe { crate::esp32::esp_idf_svc::sys::esp_camera_init(&self.config) })
            as crate::esp32::esp_idf_svc::sys::esp_err_t;
        let ret = crate::esp32::esp_idf_svc::sys::EspError::convert(ret);
        ret.map_err(|e| CameraError::CameraInitError(e.into()))
    }
    pub fn get_cam_frame(&self) -> Option<*mut crate::esp32::esp_idf_svc::sys::camera_fb_t> {
        let ptr = (unsafe { crate::esp32::esp_idf_svc::sys::esp_camera_fb_get() })
            as *mut crate::esp32::esp_idf_svc::sys::camera_fb_t;
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    pub fn return_cam_frame(
        &self,
        frame: Option<*mut crate::esp32::esp_idf_svc::sys::camera_fb_t>,
    ) {
        if let Some(ptr) = frame {
            unsafe { crate::esp32::esp_idf_svc::sys::esp_camera_fb_return(ptr) }
        }
    }
    pub fn debug_print_fb(&self, frame: &Option<*mut crate::esp32::esp_idf_svc::sys::camera_fb_t>) {
        if let Some(ptr) = frame {
            let ptr = ptr as &*mut crate::esp32::esp_idf_svc::sys::camera_fb_t;
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
    fn get_frame(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        if let Some(ptr) = self.get_cam_frame() {
            let buf = unsafe {
                let buf = (*ptr).buf;
                let len = (*ptr).len as usize;
                core::slice::from_raw_parts(buf, len)
            };
            if buf.len() > buffer.capacity() {
                self.return_cam_frame(Some(ptr));
                return Err(CameraError::CameraFrameTooBig);
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
        Err(CameraError::CameraCouldntGetFrame)
    }
}
*/
