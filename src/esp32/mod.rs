//! ESP32-specific implementations of common components

pub mod analog;
pub mod base;
pub mod board;
#[cfg(feature = "camera")]
pub mod camera;
pub mod certificate;
pub mod dtls;
pub mod encoder;
pub mod entry;
pub mod exec;
pub mod i2c;
pub mod pin;
pub mod pulse_counter;
pub mod pwm;
pub mod single_encoded_motor;
pub mod single_encoder;
pub mod tcp;
pub mod tls;
pub mod utils;
pub mod webhook;
pub mod conn {
    pub mod mdns;
}
