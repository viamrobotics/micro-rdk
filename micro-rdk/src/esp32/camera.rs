#![allow(dead_code)]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::{
    common::{
        camera::{Camera, CameraError, CameraType},
        config::ConfigType,
        registry::{ComponentRegistry, Dependency},
        status::{Status, StatusError},
    },
    esp32::esp_idf_svc::sys::{
        camera::{
            camera_config_t, camera_config_t__bindgen_ty_1, camera_config_t__bindgen_ty_2,
            camera_fb_t, esp_camera_deinit, esp_camera_fb_get, esp_camera_fb_return,
            esp_camera_init,
        },
        esp,
    },
    google::{self, api::HttpBody},
    proto::component::camera,
};
use bytes::{Bytes, BytesMut};
use prost::Message;
pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_camera("esp32camera", &Esp32Camera::from_config)
        .is_err()
    {
        log::error!("esp32camera type is already registered");
    }
}

enum PixelFormat {
    /// 2BPP
    RGB565 = 0,
    /// 2BPP
    YUV422 = 1,
    /// 1.5BPP
    YUV420 = 2,
    /// 1BPP
    GRAYSCALE = 3,
    /// JPEG/Compressed
    JPEG = 4,
    /// 3BPP
    RGB888 = 5,
    RAW = 6,
    /// 3BP2P
    RGB444 = 7,
    /// 3BP2P
    RGB555 = 8,
}

/// Sizeof output image: QVGA|CIF|VGA|SVGA|XGA|SXGA|UXGA
/// as u32
enum FrameSize {
    /// 96x96
    W96XH96 = 0,
    /// 160x120
    QQVGA = 1,
    /// 176x144
    QCIF = 2,
    /// 240x176
    HQVGA = 3,
    /// 240x240
    W240XH240 = 4,
    /// 320x240
    QVGA = 5,
    /// 400x296
    CIF = 6,
    /// 480x320
    HVGA = 7,
    /// 640x480
    VGA = 8,
}

#[derive(DoCommand)]
pub struct Esp32Camera {
    config: camera_config_t,
    last_grab: Instant,
}

impl Esp32Camera {
    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<CameraType, CameraError> {
        let pin_pwdn = cfg.get_attribute::<i32>("pin_pwdn").unwrap_or(-1);
        let pin_reset = cfg.get_attribute::<i32>("pin_reset").unwrap_or(-1);
        let pin_xclk = cfg.get_attribute::<i32>("pin_xclk").unwrap_or(21);
        let pin_sccb_sda = cfg.get_attribute::<i32>("pin_sccb_sda").unwrap_or(26);
        let pin_sccb_scl = cfg.get_attribute::<i32>("pin_sccb_scl").unwrap_or(27);
        let pin_d7 = cfg.get_attribute::<i32>("pin_d7").unwrap_or(35);
        let pin_d6 = cfg.get_attribute::<i32>("pin_d6").unwrap_or(34);
        let pin_d5 = cfg.get_attribute::<i32>("pin_d5").unwrap_or(39);
        let pin_d4 = cfg.get_attribute::<i32>("pin_d4").unwrap_or(36);
        let pin_d3 = cfg.get_attribute::<i32>("pin_d3").unwrap_or(19);
        let pin_d2 = cfg.get_attribute::<i32>("pin_d2").unwrap_or(18);
        let pin_d1 = cfg.get_attribute::<i32>("pin_d1").unwrap_or(5);
        let pin_d0 = cfg.get_attribute::<i32>("pin_d0").unwrap_or(4);
        let pin_vsync = cfg.get_attribute::<i32>("pin_vsync").unwrap_or(25);
        let pin_href = cfg.get_attribute::<i32>("pin_href").unwrap_or(23);
        let pin_pclk = cfg.get_attribute::<i32>("pin_pclk").unwrap_or(22);
        let xclk_freq_hz = cfg.get_attribute::<i32>("xclk_freq_hz").unwrap_or(20000000);
        let ledc_timer = cfg.get_attribute::<u32>("ledc_timer").unwrap_or(1);
        let ledc_channel = cfg.get_attribute::<u32>("ledc_channel").unwrap_or(1);
        let pixel_format = cfg
            .get_attribute::<u32>("pixel_format")
            .unwrap_or(PixelFormat::JPEG as u32);
        let frame_size = cfg
            .get_attribute::<u32>("frame_size")
            .unwrap_or(FrameSize::W240XH240 as u32);
        // Quality of JPEG output: 0-63 lower means higher quality
        let jpeg_quality = cfg.get_attribute::<i32>("jpeg_quality").unwrap_or(32);
        // let sccb_i2c_port = cfg.get_attribute::<i32>("sccb_i2c_port").unwrap_or(0);

        let cam = Self {
            config: camera_config_t {
                pin_pwdn,
                pin_reset,
                pin_xclk,
                __bindgen_anon_1: camera_config_t__bindgen_ty_1 { pin_sccb_sda },
                __bindgen_anon_2: camera_config_t__bindgen_ty_2 { pin_sccb_scl },
                pin_d7,
                pin_d6,
                pin_d5,
                pin_d4,
                pin_d3,
                pin_d2,
                pin_d1,
                pin_d0,
                pin_vsync,
                pin_href,
                pin_pclk,
                xclk_freq_hz,
                ledc_channel,
                ledc_timer,
                pixel_format,
                frame_size,
                jpeg_quality,
                // Number of frame buffers to be allocated.
                // If more than one, then each frame will be acquired (double speed)
                fb_count: 1,
                grab_mode: 0,
                fb_location: 0,
                sccb_i2c_port: -1,
            },
            last_grab: Instant::now(),
        };

        esp!(unsafe { esp_camera_init(&cam.config) })
            .map_err(|e| CameraError::InitError(Box::new(e)))?;
        Ok(Arc::new(Mutex::new(cam)))
    }

    fn get_frame(&mut self) -> Result<Esp32CameraFrameBuffer, CameraError> {
        self.last_grab = Instant::now();
        Esp32CameraFrameBuffer::get().ok_or(CameraError::FailedToGetImage)
    }
}

impl Camera for Esp32Camera {
    fn get_image(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        let frame = self.get_frame()?;
        if frame.len() > buffer.capacity() {
            return Err(CameraError::ImageTooBig);
        }
        self.last_grab = Instant::now();
        let msg = camera::v1::GetImageResponse {
            mime_type: "image/jpeg".to_string(),
            image: frame.as_bytes(),
        };
        // safety: message must be encoded before the frame is dropped from scope
        msg.encode(&mut buffer).unwrap();

        return Ok(buffer);
    }

    fn render_frame(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        let frame = self.get_frame()?;
        if frame.len() > buffer.capacity() {
            return Err(CameraError::ImageTooBig);
        }
        let msg = HttpBody {
            content_type: "image/jpeg".to_string(),
            data: frame.as_bytes().to_vec(),
            ..Default::default()
        };
        // safety: message must be encoded before the frame is dropped from scope
        msg.encode(&mut buffer).unwrap();
        return Ok(buffer);
    }
}

impl Drop for Esp32Camera {
    fn drop(&mut self) {
        unsafe { esp_camera_deinit() };
    }
}

impl Status for Esp32Camera {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

/// https://github.com/espressif/esp32-camera/blob/28296929286584d38e0a9e3456029204898a59a7/driver/include/esp_camera.h#L163
/// typedef struct {
///    uint8_t * buf;              /*!< Pointer to the pixel data */
///    size_t len;                 /*!< Length of the buffer in bytes */
///    size_t width;               /*!< Width of the buffer in pixels */
///    size_t height;              /*!< Height of the buffer in pixels */
///    pixformat_t format;         /*!< Format of the pixel data */
///    struct timeval timestamp;   /*!< Timestamp since boot of the first DMA buffer of the frame */
/// } camera_fb_t;
struct Esp32CameraFrameBuffer(*mut camera_fb_t);

impl Esp32CameraFrameBuffer {
    fn get() -> Option<Self> {
        let ptr = (unsafe { esp_camera_fb_get() }) as *mut camera_fb_t;

        if ptr.is_null() {
            return None;
        }
        Some(Self(ptr))
    }
    fn len(&self) -> usize {
        unsafe { (*(self.0)).len as usize }
    }
    fn width(&self) -> usize {
        unsafe { (*(self.0)).width }
    }
    fn height(&self) -> usize {
        unsafe { (*(self.0)).height }
    }
    fn format(&self) -> u32 {
        unsafe { (*(self.0)).format }
    }
    fn buf(&self) -> *const u8 {
        unsafe { (*(self.0)).buf }
    }
    fn as_bytes(self) -> Bytes {
        let buf = unsafe { core::slice::from_raw_parts(self.buf(), self.len()) };
        Bytes::from(buf)
    }
}

impl Drop for Esp32CameraFrameBuffer {
    fn drop(&mut self) {
        unsafe { esp_camera_fb_return(self.0) }
    }
}
