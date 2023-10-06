//! Base structs and methods for supported motors.
//!
//! # Creating a `PwmDirectionMotorEsp32` Motor
//!
//! ```ignore
//! use esp_idf_hal::ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver};
//! use micro_rdk::esp32::motor::PwmDirectionMotorEsp32;
//! use esp_idf_hal::units::FromValueType;
//!
//! let tconf = TimerConfig::default().frequency(10.kHz().into());
//! let timer = Arc::new(LedcTimerDriver::new(periph.ledc.channel0, &tconf)).unwrap();
//! let pwm = LedcDriver::new(periph.ledc.channel0, timer.clone(), periph.pins.gpio15)?;
//! let mut motor = PwmDirectionMotorEsp32::new(
//!     PinDriver::output(periph.pins.gpio23)?, // DIR
//!     pwm, // PWM
//!     true, // dir_flip
//! );
//!
//! motor.set_power(1.0).unwrap();
//!
//! ```
//!
//! # Creating a Robot with a Motor
//! ```ignore
//! let mut res: micro_rdk::common::robot::ResourceMap = HashMap::with_capacity(1);
//!
//! res.insert(
//!     ResourceName {
//!         namespace: "rdk".to_string(),
//!         r#type: "component".to_string(),
//!         subtype: "motor".to_string(),
//!         name: "left-motor"
//!     },
//!     ResourceType::Motor(Arc::new(Mutex::new(motor))),
//! );
//!
//! let robot_with_motor = LocalRobot(res);
//!
//! ```
//!
#![allow(dead_code)]
use esp_idf_hal::gpio::{AnyIOPin, AnyOutputPin, Output, PinDriver};

use super::pin::PinExt;
use super::pwm::{create_pwm_driver, PwmDriver};
use crate::common::config::ConfigType;
use crate::common::encoder::{
    Encoder, EncoderPositionType, EncoderType, COMPONENT_NAME as EncoderCompName,
};
use crate::common::math_utils::go_for_math;
use crate::common::motor::{
    Motor, MotorPinType, MotorPinsConfig, MotorType, COMPONENT_NAME as MotorCompName,
};
use crate::common::registry::{ComponentRegistry, Dependency, ResourceKey};
use crate::common::robot::Resource;
use crate::common::status::Status;
use crate::common::stop::Stoppable;
use crate::google;

use log::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use embedded_hal::digital::v2::OutputPin;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_motor("gpio", &gpio_motor_from_config)
        .is_err()
    {
        log::error!("gpio model is already registered")
    }
    if registry
        .register_dependency_getter(
            MotorCompName,
            "gpio",
            &ABMotorEsp32::<
                PinDriver<'_, AnyOutputPin, Output>,
                PinDriver<'_, AnyOutputPin, Output>,
            >::dependencies_from_config,
        )
        .is_err()
    {
        log::error!("failed to register dependency getter for gpio model")
    }
}

// Generates a motor or an encoded motor depending on whether an encoder has been added as
// a dependency from the config. TODO: Add support for initializing PwmDirectionMotorEsp32.
pub fn gpio_motor_from_config(cfg: ConfigType, deps: Vec<Dependency>) -> anyhow::Result<MotorType> {
    let motor_type = if let Ok(pin_cfg) = cfg.get_attribute::<MotorPinsConfig>("pins") {
        pin_cfg.detect_motor_type()?
    } else {
        return Err(anyhow::anyhow!("pin parameters for motor not found"));
    };
    let motor = match motor_type {
        MotorPinType::PwmAB => ABMotorEsp32::<
            PinDriver<'_, AnyOutputPin, Output>,
            PinDriver<'_, AnyOutputPin, Output>,
        >::from_config(cfg)?
        .clone(),
        MotorPinType::PwmDirection => {
            PwmDirectionMotorEsp32::<PinDriver<'_, AnyOutputPin, Output>>::from_config(cfg)?.clone()
        }
    };
    let mut enc: Option<EncoderType> = None;
    for Dependency(_, dep) in deps {
        match dep {
            Resource::Encoder(found_enc) => {
                enc = Some(found_enc.clone());
                break;
            }
            _ => {
                continue;
            }
        };
    }
    if let Some(enc) = enc {
        let enc_motor = EncodedMotor::new(motor, enc.clone());
        return Ok(Arc::new(Mutex::new(enc_motor)));
    }
    Ok(motor)
}

pub struct EncodedMotor<M, Enc> {
    motor: M,
    enc: Enc,
}

impl<M, Enc> EncodedMotor<M, Enc>
where
    M: Motor,
    Enc: Encoder,
{
    pub fn new(motor: M, enc: Enc) -> Self {
        Self { motor, enc }
    }
}

impl<M, Enc> Motor for EncodedMotor<M, Enc>
where
    M: Motor,
    Enc: Encoder,
{
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(self
            .enc
            .get_position(EncoderPositionType::UNSPECIFIED)?
            .value as i32)
    }

    /// Accepts percentage as a float, e.g. `0.5` equals `50%` power.
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        self.motor.set_power(pct)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        self.motor.go_for(rpm, revolutions)
    }
}

impl<M, Enc> Stoppable for EncodedMotor<M, Enc>
where
    M: Motor,
    Enc: Encoder,
{
    fn stop(&mut self) -> anyhow::Result<()> {
        self.set_power(0.0)
    }
}

impl<M, Enc> Status for EncodedMotor<M, Enc>
where
    M: Motor,
    Enc: Encoder,
{
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut hm = HashMap::new();
        let pos = self
            .enc
            .get_position(EncoderPositionType::UNSPECIFIED)?
            .value as f64;
        hm.insert(
            "position".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}

// Represents a motor using a A, B, and PWM pins
pub struct ABMotorEsp32<A, B> {
    a: A,
    b: B,
    pwm_driver: PwmDriver<'static>,
    max_rpm: f64,
    dir_flip: bool,
}

impl<A, B> ABMotorEsp32<A, B>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
{
    pub fn new(a: A, b: B, pwm: AnyIOPin, max_rpm: f64, dir_flip: bool) -> anyhow::Result<Self> {
        let pwm_driver = create_pwm_driver(pwm, 10000)?;
        Ok(Self {
            a,
            b,
            pwm_driver,
            max_rpm,
            dir_flip,
        })
    }

    pub(crate) fn dependencies_from_config(cfg: ConfigType) -> Vec<ResourceKey> {
        let mut r_keys = Vec::new();
        if let Ok(enc_name) = cfg.get_attribute::<String>("encoder") {
            let r_key = ResourceKey(EncoderCompName, enc_name);
            r_keys.push(r_key)
        }
        r_keys
    }

    pub(crate) fn from_config(cfg: ConfigType) -> anyhow::Result<MotorType> {
        if let Ok(pins) = cfg.get_attribute::<MotorPinsConfig>("pins") {
            if pins.a.is_some() && pins.b.is_some() {
                let pwm_pin = unsafe { AnyIOPin::new(pins.pwm) };
                let max_rpm: f64 = cfg.get_attribute::<f64>("max_rpm")?;
                let dir_flip: bool = cfg.get_attribute::<bool>("dir_flip").unwrap_or_default();
                return Ok(Arc::new(Mutex::new(ABMotorEsp32::new(
                    PinDriver::output(unsafe { AnyOutputPin::new(pins.a.unwrap()) })?,
                    PinDriver::output(unsafe { AnyOutputPin::new(pins.b.unwrap()) })?,
                    pwm_pin,
                    max_rpm,
                    dir_flip,
                )?)));
            }
        }
        Err(anyhow::anyhow!("cannot build motor"))
    }
}

impl<A, B> Motor for ABMotorEsp32<A, B>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
{
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        if !(-1.0..=1.0).contains(&pct) {
            anyhow::bail!("power outside limit")
        }
        let set_forwards = (pct > 0.0) && !self.dir_flip;
        if set_forwards {
            self.a
                .set_low()
                .map_err(|_| anyhow::anyhow!("error setting A pin"))?;
            self.b
                .set_high()
                .map_err(|_| anyhow::anyhow!("error setting B pin"))?;
        } else {
            self.a
                .set_high()
                .map_err(|_| anyhow::anyhow!("error setting A pin"))?;
            self.b
                .set_low()
                .map_err(|_| anyhow::anyhow!("error setting B pin"))?;
        }

        self.pwm_driver.set_ledc_duty_pct(pct)?;
        debug!("set to {:?} pct", pct);
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(0)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        let (pwr, dur) = go_for_math(self.max_rpm, rpm, revolutions)?;
        self.set_power(pwr)?;
        if dur.is_some() {
            return Ok(dur);
        }
        Ok(None)
    }
}

impl<A, B> Status for ABMotorEsp32<A, B>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
{
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut hm = HashMap::new();
        let pos = 0.0;
        hm.insert(
            "position".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}

impl<A, B> Stoppable for ABMotorEsp32<A, B>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
{
    fn stop(&mut self) -> anyhow::Result<()> {
        self.set_power(0.0)
    }
}

// Represents a motor using a direction pin and a PWM pin
pub struct PwmDirectionMotorEsp32<DIR> {
    dir: DIR,
    pwm_driver: PwmDriver<'static>,
    dir_flip: bool,
    max_rpm: f64,
}

impl<DIR> PwmDirectionMotorEsp32<DIR>
where
    DIR: OutputPin + PinExt,
{
    pub fn new(dir: DIR, pwm: AnyIOPin, dir_flip: bool, max_rpm: f64) -> anyhow::Result<Self> {
        let pwm_driver = create_pwm_driver(pwm, 10000)?;
        Ok(Self {
            dir,
            pwm_driver,
            dir_flip,
            max_rpm,
        })
    }

    pub(crate) fn from_config(cfg: ConfigType) -> anyhow::Result<MotorType> {
        if let Ok(pins) = cfg.get_attribute::<MotorPinsConfig>("pins") {
            if pins.dir.is_some() {
                let pwm_pin = unsafe { AnyIOPin::new(pins.pwm) };
                let max_rpm: f64 = cfg.get_attribute::<f64>("max_rpm")?;
                let dir_flip: bool = cfg.get_attribute::<bool>("dir_flip").unwrap_or_default();
                return Ok(Arc::new(Mutex::new(PwmDirectionMotorEsp32::new(
                    PinDriver::output(unsafe { AnyOutputPin::new(pins.dir.unwrap()) })?,
                    pwm_pin,
                    dir_flip,
                    max_rpm,
                )?)));
            }
        }
        Err(anyhow::anyhow!("cannot build motor"))
    }
}

impl<DIR> Motor for PwmDirectionMotorEsp32<DIR>
where
    DIR: OutputPin + PinExt,
{
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        if !(-1.0..=1.0).contains(&pct) {
            anyhow::bail!("power outside limit")
        }
        let set_high = (pct > 0.0) && !self.dir_flip;
        if set_high {
            self.dir
                .set_high()
                .map_err(|_| anyhow::anyhow!("error setting direction pin"))?;
        } else {
            self.dir
                .set_low()
                .map_err(|_| anyhow::anyhow!("error setting direction pin"))?;
        }
        self.pwm_driver.set_ledc_duty_pct(pct)?;
        debug!("set to {:?} pct", pct);
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(0)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        let (pwr, dur) = go_for_math(self.max_rpm, rpm, revolutions)?;
        self.set_power(pwr)?;
        if dur.is_some() {
            return Ok(dur);
        }
        Ok(None)
    }
}

impl<DIR> Status for PwmDirectionMotorEsp32<DIR>
where
    DIR: OutputPin + PinExt,
{
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut hm = HashMap::new();
        let pos = 0.0;
        hm.insert(
            "position".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}

impl<DIR> Stoppable for PwmDirectionMotorEsp32<DIR>
where
    DIR: OutputPin + PinExt,
{
    fn stop(&mut self) -> anyhow::Result<()> {
        self.set_power(0.0)
    }
}
