//! Base structs and methods for supported motors.
//!
//! # Creating a `PwmDirectionMotor` Motor
//!
//! ```ignore
//!
//! let board = FakeBoard::new(vec![]);
//!
//! let mut motor = PwmDirectionMotor::new(
//!     12, // direction pin
//!     32, // PWM pin
//!     true, // dir_flip
//!     100, // max_rpm
//!     board,
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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;

use super::board::{Board, BoardType};
use super::config::ConfigType;
use super::encoder::{
    Encoder, EncoderPositionType, EncoderType, COMPONENT_NAME as EncoderCompName,
};
use super::math_utils::go_for_math;
use super::motor::{
    Motor, MotorPinType, MotorPinsConfig, MotorType, COMPONENT_NAME as MotorCompName,
};
use super::registry::{get_board_from_dependencies, ComponentRegistry, Dependency, ResourceKey};
use super::robot::Resource;
use super::status::Status;
use super::stop::Stoppable;
use crate::google;

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
            &PwmABMotor::<BoardType>::dependencies_from_config,
        )
        .is_err()
    {
        log::error!("failed to register dependency getter for gpio model")
    }
}

// Generates a motor or an encoded motor depending on whether an encoder has been added as
// a dependency from the config.
pub(crate) fn gpio_motor_from_config(
    cfg: ConfigType,
    deps: Vec<Dependency>,
) -> anyhow::Result<MotorType> {
    let mut enc: Option<EncoderType> = None;
    for Dependency(_, dep) in &deps {
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
    let board = get_board_from_dependencies(deps)
        .context("gpio motor requires a board in its dependencies")?;
    let motor_type = if let Ok(pin_cfg) = cfg.get_attribute::<MotorPinsConfig>("pins") {
        pin_cfg.detect_motor_type()?
    } else {
        return Err(anyhow::anyhow!("pin parameters for motor not found"));
    };
    let motor = match motor_type {
        MotorPinType::PwmAB => PwmABMotor::<BoardType>::from_config(cfg, board.clone())?.clone(),
        MotorPinType::PwmDirection => {
            PwmDirectionMotor::<BoardType>::from_config(cfg, board.clone())?.clone()
        }
    };
    if let Some(enc) = enc {
        let enc_motor = EncodedMotor::new(motor, enc.clone());
        return Ok(Arc::new(Mutex::new(enc_motor)));
    }
    Ok(motor)
}

// Motors generally don't care about the PWM frequency, so long as
// it is in the order of magnitude 0f 10s of kHZ. For simplicity, we
// just select 10kHz.
const MOTOR_PWM_FREQUENCY: u64 = 10000;

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
        self.motor.stop()
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
pub(crate) struct PwmABMotor<B> {
    board: B,
    a_pin: i32,
    b_pin: i32,
    pwm_pin: i32,
    max_rpm: f64,
    dir_flip: bool,
}

impl<B> PwmABMotor<B>
where
    B: Board,
{
    pub(crate) fn new(
        a_pin: i32,
        b_pin: i32,
        pwm_pin: i32,
        max_rpm: f64,
        dir_flip: bool,
        board: B,
    ) -> anyhow::Result<Self> {
        let mut res = Self {
            board,
            a_pin,
            b_pin,
            pwm_pin,
            max_rpm,
            dir_flip,
        };
        res.board.set_pwm_frequency(pwm_pin, MOTOR_PWM_FREQUENCY)?;
        Ok(res)
    }

    pub(crate) fn dependencies_from_config(cfg: ConfigType) -> Vec<ResourceKey> {
        let mut r_keys = Vec::new();
        if let Ok(enc_name) = cfg.get_attribute::<String>("encoder") {
            let r_key = ResourceKey(EncoderCompName, enc_name);
            r_keys.push(r_key)
        }
        r_keys
    }

    pub(crate) fn from_config(cfg: ConfigType, board: BoardType) -> anyhow::Result<MotorType> {
        if let Ok(pins) = cfg.get_attribute::<MotorPinsConfig>("pins") {
            if pins.a.is_some() && pins.b.is_some() {
                let pwm_pin = pins.pwm;
                let a_pin = pins.a.unwrap();
                let b_pin = pins.b.unwrap();
                let max_rpm: f64 = cfg.get_attribute::<f64>("max_rpm")?;
                let dir_flip: bool = cfg.get_attribute::<bool>("dir_flip").unwrap_or_default();
                return Ok(Arc::new(Mutex::new(PwmABMotor::new(
                    a_pin, b_pin, pwm_pin, max_rpm, dir_flip, board,
                )?)));
            }
        }
        Err(anyhow::anyhow!("cannot build motor"))
    }
}

impl<B> Motor for PwmABMotor<B>
where
    B: Board,
{
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        if !(-1.0..=1.0).contains(&pct) {
            anyhow::bail!("power outside limit, must be between -1.0 and 1.0")
        }
        let set_forwards = (pct > 0.0) && !self.dir_flip;
        if set_forwards {
            self.board.set_gpio_pin_level(self.a_pin, false)?;
            self.board.set_gpio_pin_level(self.b_pin, true)?;
        } else {
            self.board.set_gpio_pin_level(self.a_pin, true)?;
            self.board.set_gpio_pin_level(self.b_pin, false)?;
        }
        self.board.set_pwm_duty(self.pwm_pin, pct)?;
        Ok(())
    }

    fn get_position(&mut self) -> anyhow::Result<i32> {
        anyhow::bail!("position reporting not supported without an encoder")
    }

    fn go_for(
        &mut self,
        rpm: f64,
        revolutions: f64,
    ) -> anyhow::Result<Option<std::time::Duration>> {
        let (pwr, dur) = go_for_math(self.max_rpm, rpm, revolutions)?;
        self.set_power(pwr)?;
        if dur.is_some() {
            return Ok(dur);
        }
        Ok(None)
    }
}

impl<B> Status for PwmABMotor<B>
where
    B: Board,
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

impl<B> Stoppable for PwmABMotor<B>
where
    B: Board,
{
    fn stop(&mut self) -> anyhow::Result<()> {
        self.set_power(0.0)
    }
}

// Represents a motor using a direction pin and a PWM pin
pub(crate) struct PwmDirectionMotor<B> {
    board: B,
    dir_pin: i32,
    pwm_pin: i32,
    max_rpm: f64,
    dir_flip: bool,
}

impl<B> PwmDirectionMotor<B>
where
    B: Board,
{
    pub(crate) fn new(
        dir_pin: i32,
        pwm_pin: i32,
        max_rpm: f64,
        dir_flip: bool,
        board: B,
    ) -> anyhow::Result<Self> {
        let mut res = Self {
            board,
            dir_pin,
            pwm_pin,
            max_rpm,
            dir_flip,
        };
        res.board.set_pwm_frequency(pwm_pin, MOTOR_PWM_FREQUENCY)?;
        Ok(res)
    }

    pub(crate) fn from_config(cfg: ConfigType, board: BoardType) -> anyhow::Result<MotorType> {
        if let Ok(pins) = cfg.get_attribute::<MotorPinsConfig>("pins") {
            if let Some(dir_pin) = pins.dir {
                let pwm_pin = pins.pwm;
                let max_rpm: f64 = cfg.get_attribute::<f64>("max_rpm")?;
                let dir_flip: bool = cfg.get_attribute::<bool>("dir_flip").unwrap_or_default();
                return Ok(Arc::new(Mutex::new(PwmDirectionMotor::new(
                    dir_pin, pwm_pin, max_rpm, dir_flip, board,
                )?)));
            }
        }
        Err(anyhow::anyhow!("cannot build motor"))
    }
}

impl<B> Motor for PwmDirectionMotor<B>
where
    B: Board,
{
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        if !(-1.0..=1.0).contains(&pct) {
            anyhow::bail!("power outside limit, must be between -1.0 and 1.0")
        }
        let set_high = (pct > 0.0) && !self.dir_flip;
        self.board.set_gpio_pin_level(self.dir_pin, set_high)?;
        self.board.set_pwm_duty(self.pwm_pin, pct)?;
        Ok(())
    }

    fn get_position(&mut self) -> anyhow::Result<i32> {
        anyhow::bail!("position reporting not supported without an encoder")
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

impl<B> Status for PwmDirectionMotor<B>
where
    B: Board,
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

impl<B> Stoppable for PwmDirectionMotor<B>
where
    B: Board,
{
    fn stop(&mut self) -> anyhow::Result<()> {
        self.set_power(0.0)
    }
}
