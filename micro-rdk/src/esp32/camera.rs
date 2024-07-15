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
        EspError,
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

struct Esp32CameraConfig {
    pin_pwdn: i32,
    pin_reset: i32,
    pin_xclk: i32,
    pin_sccb_sda: i32,
    pin_sccb_scl: i32,
    pin_d7: i32,
    pin_d6: i32,
    pin_d5: i32,
    pin_d4: i32,
    pin_d3: i32,
    pin_d2: i32,
    pin_d1: i32,
    pin_d0: i32,
    pin_vsync: i32,
    pin_href: i32,
    pin_pclk: i32,
    xclk_freq_hz: i32,
    ledc_timer: u32,
    ledc_channel: u32,
    pixel_format: PixelFormat,
    frame_size: FrameSize,

    /// Quality of JPEG output: 0-63 lower means higher quality
    jpeg_quality: i32,

    /// Number of frame buffers to be allocated. If more than one, then each frame will be acquired (double speed)
    fb_count: usize,
    /// when buffers should be filled:
    grab_mode: u32,
    fb_location: u32,
    sccb_i2c_port: i32,
}

impl Esp32CameraConfig {
    fn to_conf_t(self) -> camera_config_t {
        camera_config_t {
            pin_pwdn : self.pin_pwdn,
            pin_reset,
            pin_xclk,
            __bindgen_anon_1: camera_config_t__bindgen_ty_1 { pin_sccb_sda: self.pin_sccb_sda },
            __bindgen_anon_2: camera_config_t__bindgen_ty_2 { pin_sccb_scl: self.pin_sccb_scl },
            pin_d7: self.pin_d7,
            pin_d6: self.pin_d6,
            pin_d5: self.pin_d5,
            pin_d4: self.pin_d4,
            pin_d3: self.pin_d3,
            pin_d2: self.pin_d2,
            pin_d1: self.pin_d1,
            pin_d0: self.pin_d0,
            pin_vsync: self.pin_vsync,
            pin_href: self.pin_href,
            pin_pclk: self.pin_pclk,
            xclk_freq_hz: self.xclk_freq_hz,
            ledc_timer: self.ledc_timer,
            ledc_channel: self.ledc_channel,
            pixel_format: self.pixel_format.try_into().unwrap(),
            frame_size: self.frame_size.try_into().unwrap(),
            jpeg_quality: self.jpeg_quality,
            fb_count: self.fb_count,
            grab_mode: self.grab_mode,
            fb_location: self.fb_location,
            sccb_i2c_port: self.sccb_i2c_port,
        }
    }
}

impl TryFrom<ConfigType> for Esp32CameraConfig {
    type Error = ();
    fn try_from(value: ConfigType) -> Result<Self, Self::Error> {
        Ok(Self {
            pin_pwdn : value.get_attribute::<i32>("pin_pwdn").unwrap_or(-1),
            pin_reset : value.get_attribute::<i32>("pin_reset").unwrap_or(-1),
            pin_xclk : value.get_attribute::<i32>("pin_xclk").unwrap_or(21),
            pin_sccb_sda : value.get_attribute::<i32>("pin_sccb_sda").unwrap_or(26),
            pin_sccb_scl : value.get_attribute::<i32>("pin_sccb_scl").unwrap_or(27),
            pin_d7 : value.get_attribute::<i32>("pin_d7").unwrap_or(35),
            pin_d6 : value.get_attribute::<i32>("pin_d6").unwrap_or(34),
            pin_d5 : value.get_attribute::<i32>("pin_d5").unwrap_or(39),
            pin_d4 : value.get_attribute::<i32>("pin_d4").unwrap_or(36),
            pin_d3 : value.get_attribute::<i32>("pin_d3").unwrap_or(19),
            pin_d2 : value.get_attribute::<i32>("pin_d2").unwrap_or(18),
            pin_d1 : value.get_attribute::<i32>("pin_d1").unwrap_or(5),
            pin_d0 : value.get_attribute::<i32>("pin_d0").unwrap_or(4),
            pin_vsync : value.get_attribute::<i32>("pin_vsync").unwrap_or(25),
            pin_href : value.get_attribute::<i32>("pin_href").unwrap_or(23),
            pin_pclk : value.get_attribute::<i32>("pin_pclk").unwrap_or(22),
            xclk_freq_hz : value.get_attribute::<i32>("xclk_freq_hz").unwrap_or(20000000),
            ledc_timer : value.get_attribute::<u32>("ledc_timer").unwrap_or(1),
            ledc_channel : value.get_attribute::<u32>("ledc_channel").unwrap_or(1),
            pixel_format : value.get_attribute::<u32>("pixel_format").unwrap_or(PixelFormat::JPEG as u32).try_into().unwrap(),
            frame_size : value.get_attribute::<u32>("frame_size").unwrap_or(FrameSize::VGA as u32).try_into().unwrap(),
            // Quality of JPEG output: 0-63 lower means higher quality
            jpeg_quality : value.get_attribute::<i32>("jpeg_quality").unwrap_or(32),
            // Number of frame buffers to be allocated. If more than one, then each frame will be acquired (double speed)
            fb_count : value.get_attribute::<u32>("fb_count").unwrap_or(1) as usize,
            grab_mode : value.get_attribute::<u32>("grab_mode").unwrap_or(0),
            fb_location : value.get_attribute::<u32>("fb_location").unwrap_or(0),
            sccb_i2c_port : value.get_attribute::<i32>("sccb_i2c_port").unwrap_or(0),
        })
    }
}

enum PixelFormat {
    RGB565 = 0,    // 2BPP/RGB565
    YUV422 = 1,    // 2BPP/YUV422
    YUV420 = 2,    // 1.5BPP/YUV420
    GRAYSCALE = 3, // 1BPP/GRAYSCALE
    JPEG = 4,      // JPEG/COMPRESSED
    RGB888 = 5,    // 3BPP/RGB888
    RAW = 6,       // RAW
    RGB444 = 7,    // 3BP2P/RGB444
    RGB555 = 8,    // 3BP2P/RGB555
}

impl TryFrom<u32> for PixelFormat {
    type Error = ();
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::RGB565),
            1 => Ok(Self::YUV422),
            2 => Ok(Self::YUV420),
            3 => Ok(Self::GRAYSCALE),
            4 => Ok(Self::JPEG),
            5 => Ok(Self::RGB888),
            6 => Ok(Self::RAW),
            7 => Ok(Self::RGB444),
            8 => Ok(Self::RGB555),
            _ => Err(()),
        }
    }
}

/// Sizeof output image: QVGA|CIF|VGA|SVGA|XGA|SXGA|UXGA
/// as u32
enum FrameSize {
        96X96 = 0,    // 96x96
        QQVGA = 1,    // 160x120 
        QCIF = 2,     // 176x144 
        HQVGA = 3,    // 240x176 
        240X240 = 4,  // 240x240 
        QVGA = 5,     // 320x240 
        CIF = 6,      // 400x296 
        HVGA = 7,     // 480x320 
        VGA = 8,      // 640x480 
}

impl TryFrom<u32> for FrameSize {
    type Error = ();
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::96X96),
            1 => Ok(Self::QQVGA),
            2 => Ok(Self::QCIF),
            3 => Ok(Self::HQVGA),
            4 => Ok(Self::240X240),
            5 => Ok(Self::QVGA),
            6 => Ok(Self::CIF),
            7 => Ok(Self::HVGA),
            8 => Ok(Self::VGA),
            _ => Err(()),
        }
    }
}

#[derive(DoCommand)]
pub struct Esp32Camera {
    config: Esp32CameraConfig,
    last_grab: Instant,
}

impl Esp32Camera {
    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<CameraType, CameraError> {

        let config = Esp32CameraConfig::try_from(cfg).unwrap();
        Ok(Arc::new(Mutex::new(Self::init(config)?)))
    }

    pub fn init(config: Esp32CameraConfig) -> Result<Self, CameraError> {
        let conf = config.to_conf_t();
        let ret =
            (unsafe { esp_camera_init(&conf as *const camera_config_t) }) as crate::esp32::esp_idf_svc::sys::esp_err_t;
        EspError::convert(ret).map_err(|e| CameraError::InitError(e.into()))?;
        Ok(Self {
            config,
            last_grab: Instant::now(),
        })
    }
    fn get_frame(&mut self) -> Result<Esp32CameraFrameBuffer, CameraError> {
        self.last_grab = Instant::now();
        let frame = Esp32CameraFrameBuffer::get().ok_or_else(|| CameraError::FailedToGetImage)?;
        log::info!(
            "width: {} - height: {} - pixformat: {}",
            frame.width(),
            frame.height(),
            frame.format()
        );
        Ok(frame)
    }
}

impl Camera for Esp32Camera {
    fn get_image(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        if let Ok(frame) = self.get_frame() {
            if frame.len() > buffer.capacity() {
                return Err(CameraError::ImageTooBig);
            }
            let msg = camera::v1::GetImageResponse {
                mime_type: "image/jpeg".to_string(),
                image: unsafe { frame.as_bytes() },
            };
            // safety: message must be encoded before the frame is dropped from scope
            msg.encode(&mut buffer).unwrap();

            return Ok(buffer);
        }
        Err(CameraError::FailedToGetImage)
    }

    fn render_frame(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        if let Ok(frame) = self.get_frame() {
            if frame.len() > buffer.capacity() {
                return Err(CameraError::ImageTooBig);
            }
            let msg = HttpBody {
                content_type: "image/jpeg".to_string(),
                data: unsafe { frame.as_bytes().to_vec() },
                ..Default::default()
            };
            // safety: message must be encoded before the frame is dropped from scope
            msg.encode(&mut buffer).unwrap();
            return Ok(buffer);
        }
        Err(CameraError::FailedToGetImage)
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
    unsafe fn buf(&self) -> *const u8 {
        (*(self.0)).buf
    }
    unsafe fn as_bytes(self) -> Bytes {
        let buf = unsafe { core::slice::from_raw_parts(self.buf(), self.len()) };
        Bytes::from(buf)
    }
}

impl Drop for Esp32CameraFrameBuffer {
    fn drop(&mut self) {
        unsafe { esp_camera_fb_return(self.0) }
    }
}
