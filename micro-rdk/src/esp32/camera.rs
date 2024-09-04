#![allow(dead_code)]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    common::{
        board::Board,
        camera::{Camera, CameraError, CameraType},
        config::ConfigType,
        i2c::I2cHandleType,
        registry::{get_board_from_dependencies, ComponentRegistry, Dependency},
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
        .register_camera("esp32-camera", &Esp32Camera::from_config)
        .is_err()
    {
        log::error!("esp32camera type is already registered");
    }
}

static CAMERA_ALREADY_REGISTERED: Mutex<bool> = Mutex::new(false);

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
/// as u32.
enum FrameSize {
    /// 96x96, ~1.88KB JPEG
    W96XH96 = 0,
    /// 160x120, ~3.92 KB JPEG
    QQVGA = 1,
    /// 176x144, ~5.17 KB JPEG
    QCIF = 2,
    /// 240x176, ~8.62KB JPEG
    HQVGA = 3,
    /// 240x240, ~11.76KB JPEG
    W240XH240 = 4,
    /// 320x240, ~15.68 KB JPEG
    QVGA = 5,
    /// 400x296, ~24.18KB JPEG
    CIF = 6,
    /// 480x320, ~31.36KB JPEG
    HVGA = 7,
    /// 640x480, ~62.73KB JPEG
    VGA = 8,
}

#[derive(DoCommand)]
pub struct Esp32Camera {
    config: camera_config_t,
    i2c_handle: I2cHandleType,
}

impl Esp32Camera {
    pub(crate) fn from_config(
        cfg: ConfigType,
        dependencies: Vec<Dependency>,
    ) -> Result<CameraType, CameraError> {
        let board = get_board_from_dependencies(dependencies);
        if board.is_none() {
            return Err(CameraError::ConfigError("Esp32Camera missing board"));
        }
        let board = board.unwrap();
        let i2c_handle: I2cHandleType;
        if let Ok(i2c_name) = cfg.get_attribute::<String>("i2c_bus") {
            i2c_handle = board.get_i2c_by_name(i2c_name)?;
        } else {
            return Err(CameraError::ConfigError("Esp32Camera missing i2c_bus"));
        }

        // esp32-camera can initialize an i2c bus via `camera_config_t` and
        // `esp_camera_init`; we are choosing not exposing it, enforcing i2c bus
        // initialization through a `Board`.
        let sccb_i2c_port: i32 = i2c_handle.lock().bus_no() as i32;
        let pin_sccb_sda = -1;
        let pin_sccb_scl = -1;

        let pin_pwdn = cfg.get_attribute::<i32>("pin_pwdn").unwrap_or(-1);
        let pin_reset = cfg.get_attribute::<i32>("pin_reset").unwrap_or(-1);
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
        let pin_xclk = cfg.get_attribute::<i32>("pin_xclk").unwrap_or(21);
        let xclk_freq_hz = cfg
            .get_attribute::<i32>("xclk_freq_hz")
            .unwrap_or(20_000_000);
        let ledc_timer = cfg.get_attribute::<u32>("ledc_timer").unwrap_or(1);
        let ledc_channel = cfg.get_attribute::<u32>("ledc_channel").unwrap_or(1);
        let frame_size = cfg
            .get_attribute::<u32>("frame_size")
            .unwrap_or(FrameSize::QVGA as u32);
        // Quality of JPEG output: 0-63 lower means higher quality
        let jpeg_quality = cfg.get_attribute::<i32>("jpeg_quality").unwrap_or(32);
        //  If pin_sccb_sda is -1, use the already configured I2C bus by number
        let sccb_i2c_port = cfg.get_attribute::<i32>("sccb_i2c_port").unwrap_or(-1);

        let config = camera_config_t {
            pin_pwdn,
            pin_reset,
            pin_xclk,
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
            pixel_format: PixelFormat::JPEG as u32,
            frame_size,
            jpeg_quality,
            // Number of frame buffers to be allocated.
            // If more than one, then each frame will be acquired (double speed)
            // when pixel_format == jpeg, fb_count > 1 goes to continuous mode, may need to adjust
            // xclk_freq_hz down to 10_000_000.
            fb_count: 1,
            grab_mode: 0,
            fb_location: 0,

            __bindgen_anon_1: camera_config_t__bindgen_ty_1 { pin_sccb_sda },
            __bindgen_anon_2: camera_config_t__bindgen_ty_2 { pin_sccb_scl },
            sccb_i2c_port,
        };

        let mut registered = CAMERA_ALREADY_REGISTERED.lock().map_err(|_| {
            CameraError::InitError(
                "failed to acquire lock, another camera being initialized".into(),
            )
        })?;

        if *registered {
            return Err(CameraError::InitError(
                "only one camera allowed per machine".into(),
            ));
        }

        esp!(unsafe { esp_camera_init(&config) }).map_err(|e| {
            CameraError::InitError(format!("failed to initialize camera with config: {}", e).into())
        })?;

        *registered = true;

        Ok(Arc::new(Mutex::new(Self { config, i2c_handle })))
    }
}

impl Camera for Esp32Camera {
    fn get_image(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        let frame = Esp32CameraFrameBuffer::get().ok_or(CameraError::FailedToGetImage)?;
        if frame.len() > buffer.capacity() {
            return Err(CameraError::ImageTooBig(frame.len(), buffer.capacity()));
        }
        let msg = camera::v1::GetImageResponse {
            mime_type: "image/jpeg".to_string(),
            image: frame.as_bytes(),
        };
        // safety: message must be encoded before the frame is dropped from scope
        msg.encode(&mut buffer)
            .map_err(CameraError::MessageEncodeError)?;

        return Ok(buffer);
    }

    fn render_frame(&mut self, mut buffer: BytesMut) -> Result<BytesMut, CameraError> {
        let frame = Esp32CameraFrameBuffer::get().ok_or(CameraError::FailedToGetImage)?;
        if frame.len() > buffer.capacity() {
            return Err(CameraError::ImageTooBig(frame.len(), buffer.capacity()));
        }
        let msg = HttpBody {
            content_type: "image/jpeg".to_string(),
            data: frame.as_bytes().to_vec(),
            ..Default::default()
        };
        // safety: message must be encoded before the frame is dropped from scope
        msg.encode(&mut buffer)
            .map_err(CameraError::MessageEncodeError)?;
        return Ok(buffer);
    }
}

impl Drop for Esp32Camera {
    fn drop(&mut self) {
        let mut registered = CAMERA_ALREADY_REGISTERED.lock().unwrap();
        if *registered {
            unsafe { esp_camera_deinit() };
            *registered = false;
        }
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
