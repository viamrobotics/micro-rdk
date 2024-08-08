//! ESP32-specific implementations of components and tools

pub mod analog;
pub mod board;
#[cfg(all(feature = "camera", feature = "builtin-components"))]
pub mod camera;
pub mod certificate;
pub mod dtls;
#[cfg(feature = "builtin-components")]
pub mod encoder;
pub mod entry;
pub mod esp_idf_svc;
#[cfg(feature = "builtin-components")]
pub mod hcsr04;
pub mod i2c;
pub mod pin;
#[cfg(feature = "builtin-components")]
pub mod pulse_counter;
pub mod pwm;
#[cfg(feature = "builtin-components")]
pub mod single_encoded_motor;
#[cfg(feature = "builtin-components")]
pub mod single_encoder;
pub mod tcp;
pub mod tls;
pub mod utils;
pub mod conn {
    pub mod mdns;
    pub mod network;
}
pub mod nvs_storage;
pub mod provisioning {
    pub mod wifi_provisioning;
}
