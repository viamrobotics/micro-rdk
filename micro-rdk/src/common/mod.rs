//! Structs, traits, and utils to develop [component](https://docs.viam.com/components/)
//! drivers.
//!
//! # Components
//! - [actuator]
//! - [base]
//! - [board]
//! - [camera]
//! - [encoder]
//! - [motor]
//! - [movement_sensor]
//! - [sensor]
//! - [servo]
//!
//! # Utils
//! - [grpc]
//! - [grpc_client]
//! - [i2c]
//! - [webrtc]
//! - [conn]
//!
//!
//! General Purpose Drivers
//! - [adxl345]
//! - [gpio_motor]
//! - [ina]
//! - [mpu6050]

pub mod actuator;
#[cfg(feature = "builtin-components")]
pub mod adxl345;
pub mod analog;
pub mod app_client;
pub mod base;
pub mod board;
pub mod camera;
pub mod config;
pub mod digital_interrupt;
pub mod encoder;
pub mod entry;
pub mod generic;
#[cfg(feature = "builtin-components")]
pub mod gpio_motor;
#[cfg(feature = "builtin-components")]
pub mod gpio_servo;
pub mod grpc;
pub mod grpc_client;
pub mod i2c;
#[cfg(feature = "builtin-components")]
pub mod ina;
pub mod log;
pub mod math_utils;
#[cfg(feature = "builtin-components")]
pub mod moisture_sensor;
pub mod motor;
pub mod movement_sensor;
#[cfg(feature = "builtin-components")]
pub mod mpu6050;
pub mod power_sensor;
pub mod registry;
pub mod robot;
pub mod sensor;
pub mod servo;
pub mod status;
#[cfg(feature = "builtin-components")]
pub mod wheeled_base;
pub mod webrtc {
    pub mod api;
    pub mod candidates;
    pub mod certificate;
    pub mod dtls;
    pub mod exec;
    pub mod grpc;
    pub mod ice;
    pub mod io;
    pub mod sctp;
}
pub mod conn {
    pub mod errors;
    pub mod mdns;
    pub mod server;
    mod utils;
}
#[cfg(feature = "data")]
pub mod data_collector;
