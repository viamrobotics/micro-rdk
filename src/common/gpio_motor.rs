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

use super::actuator::Actuator;
use super::board::{Board, BoardType};
use super::config::ConfigType;
use super::encoder::{
    Encoder, EncoderPositionType, EncoderType, COMPONENT_NAME as EncoderCompName,
};
use super::math_utils::go_for_math;
use super::motor::{
    Motor, MotorPinType, MotorPinsConfig, MotorSupportedProperties, MotorType,
    COMPONENT_NAME as MotorCompName,
};
use super::registry::{get_board_from_dependencies, ComponentRegistry, Dependency, ResourceKey};
use super::robot::Resource;
use super::status::Status;
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
        MotorPinType::AB => AbMotor::<BoardType>::from_config(cfg, board.clone())?.clone(),
    };
    if let Some(enc) = enc {
        let enc_motor = EncodedMotor::new(motor, enc.clone());
        return Ok(Arc::new(Mutex::new(enc_motor)));
    }
    Ok(motor)
}

// Motors generally don't care about the PWM frequency, so long as
// it is in the order of kHZ. For simplicity, we
// just select 1 kHz. (TODO(RSDK-5619) - remove default entirely in favor
// of forcing the user to supply a PWM frequency in the motor config)
const MOTOR_PWM_FREQUENCY: u32 = 1000;

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
    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: true,
        }
    }
}

impl<M, Enc> Actuator for EncodedMotor<M, Enc>
where
    M: Motor,
    Enc: Encoder,
{
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        self.motor.is_moving()
    }
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

pub(crate) struct MotorSettings {
    max_rpm: f64,
    dir_flip: bool,
    enable_pin_high: Option<i32>,
    enable_pin_low: Option<i32>,
    pwm_frequency: u32,
}

impl MotorSettings {
    fn from_configs(cfg: &ConfigType, pins_cfg: &MotorPinsConfig) -> Self {
        let max_rpm: f64 = cfg.get_attribute::<f64>("max_rpm").unwrap_or(100.0);
        let dir_flip: bool = cfg.get_attribute::<bool>("dir_flip").unwrap_or_default();
        let enable_pin_high = pins_cfg.enable_pin_high;
        let enable_pin_low = pins_cfg.enable_pin_low;
        let pwm_frequency = pins_cfg.pwm_frequency.unwrap_or(MOTOR_PWM_FREQUENCY);
        Self {
            max_rpm,
            dir_flip,
            enable_pin_high,
            enable_pin_low,
            pwm_frequency,
        }
    }
}

// Represents a motor using a A, B, and PWM pins
pub(crate) struct PwmABMotor<B: Board> {
    board: B,
    a_pin: i32,
    b_pin: i32,
    pwm_pin: i32,
    motor_settings: MotorSettings,
}

impl<B> PwmABMotor<B>
where
    B: Board,
{
    pub(crate) fn new(
        a_pin: i32,
        b_pin: i32,
        pwm_pin: i32,
        motor_settings: MotorSettings,
        board: B,
    ) -> anyhow::Result<Self> {
        let pwm_freq = motor_settings.pwm_frequency as u64;
        let enable_high = motor_settings.enable_pin_high;
        let enable_low = motor_settings.enable_pin_low;
        let mut res = Self {
            board,
            a_pin,
            b_pin,
            pwm_pin,
            motor_settings,
        };
        // we start with this because we want to reserve a timer and PWM channel early
        // for boards where these are a limited resource
        res.board.set_pwm_frequency(pwm_pin, pwm_freq)?;

        set_enable_pins(
            &mut res.board,
            enable_high,
            enable_low,
            true,
        )?;
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
        let pins = cfg
            .get_attribute::<MotorPinsConfig>("pins")
            .or(Err(anyhow::anyhow!(
                "cannot build motor, could not find 'pins' attribute"
            )))?;
        let motor_settings = MotorSettings::from_configs(&cfg, &pins);
        let a_pin = pins
            .a
            .ok_or(anyhow::anyhow!("cannot build PwmABMotor, need 'a' pin"))?;
        let b_pin = pins
            .b
            .ok_or(anyhow::anyhow!("cannot build PwmABMotor, need 'b' pin"))?;
        let pwm_pin = pins
            .pwm
            .ok_or(anyhow::anyhow!("cannot build PwmABMotor, need PWM pin"))?;
        Ok(Arc::new(Mutex::new(PwmABMotor::new(
            a_pin,
            b_pin,
            pwm_pin,
            motor_settings,
            board,
        )?)))
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
        if pct.abs() < 0.001 {
            return self.stop();
        }
        let set_forwards = (pct > 0.0) && !self.motor_settings.dir_flip;
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
        let (pwr, dur) = go_for_math(self.motor_settings.max_rpm, rpm, revolutions)?;
        self.set_power(pwr)?;
        if dur.is_some() {
            return Ok(dur);
        }
        Ok(None)
    }

    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: false,
        }
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

impl<B> Actuator for PwmABMotor<B>
where
    B: Board,
{
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        Ok(self.board.get_pwm_duty(self.pwm_pin) <= 0.05)
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        self.board.set_gpio_pin_level(self.a_pin, false)?;
        self.board.set_gpio_pin_level(self.b_pin, false)?;
        self.board.set_pwm_duty(self.pwm_pin, 0.0)
    }
}

impl<B: Board> Drop for PwmABMotor<B> {
    fn drop(&mut self) {
        if let Err(err) = set_enable_pins(
            &mut self.board,
            self.motor_settings.enable_pin_high,
            self.motor_settings.enable_pin_low,
            false,
        ) {
            log::error!("PwmAbMotor dropped, but could not disable enable pins: {:?}", err)
        };
    }
}

// Represents a motor using a direction pin and a PWM pin
pub(crate) struct PwmDirectionMotor<B: Board> {
    board: B,
    dir_pin: i32,
    pwm_pin: i32,
    motor_settings: MotorSettings,
}

impl<B> PwmDirectionMotor<B>
where
    B: Board,
{
    pub(crate) fn new(
        dir_pin: i32,
        pwm_pin: i32,
        motor_settings: MotorSettings,
        board: B,
    ) -> anyhow::Result<Self> {
        let pwm_freq = motor_settings.pwm_frequency as u64;
        let enable_high = motor_settings.enable_pin_high;
        let enable_low = motor_settings.enable_pin_low;
        let mut res = Self {
            board,
            dir_pin,
            pwm_pin,
            motor_settings,
        };
        set_enable_pins(
            &mut res.board,
            enable_high,
            enable_low,
            true,
        )?;
        // we start with this because we want to reserve a timer and PWM channel early
        // for boards where these are a limited resource
        res.board.set_pwm_frequency(pwm_pin, pwm_freq)?;
        Ok(res)
    }

    pub(crate) fn from_config(cfg: ConfigType, board: BoardType) -> anyhow::Result<MotorType> {
        let pins = cfg
            .get_attribute::<MotorPinsConfig>("pins")
            .or(Err(anyhow::anyhow!(
                "cannot build motor, could not find 'pins' attribute"
            )))?;
        let motor_settings = MotorSettings::from_configs(&cfg, &pins);
        let dir_pin = pins.dir.ok_or(anyhow::anyhow!(
            "cannot build PwmDirectionMotor, need direction pin"
        ))?;
        let pwm_pin = pins.pwm.ok_or(anyhow::anyhow!(
            "cannot build PwmDirectionMotor, need PWM pin"
        ))?;
        Ok(Arc::new(Mutex::new(PwmDirectionMotor::new(
            dir_pin,
            pwm_pin,
            motor_settings,
            board,
        )?)))
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
        if pct.abs() < 0.001 {
            return self.stop();
        }
        let set_high = (pct > 0.0) && !self.motor_settings.dir_flip;
        self.board.set_gpio_pin_level(self.dir_pin, set_high)?;
        self.board.set_pwm_duty(self.pwm_pin, pct)?;
        Ok(())
    }

    fn get_position(&mut self) -> anyhow::Result<i32> {
        anyhow::bail!("position reporting not supported without an encoder")
    }

    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        let (pwr, dur) = go_for_math(self.motor_settings.max_rpm, rpm, revolutions)?;
        self.set_power(pwr)?;
        if dur.is_some() {
            return Ok(dur);
        }
        Ok(None)
    }

    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: false,
        }
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

impl<B> Actuator for PwmDirectionMotor<B>
where
    B: Board,
{
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        Ok(self.board.get_pwm_duty(self.pwm_pin) <= 0.05)
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        self.board.set_pwm_duty(self.pwm_pin, 0.0)
    }
}

impl<B: Board> Drop for PwmDirectionMotor<B>
{
    fn drop(&mut self) {
        if let Err(err) = set_enable_pins(
            &mut self.board,
            self.motor_settings.enable_pin_high,
            self.motor_settings.enable_pin_low,
            false,
        ) {
            log::error!("PwmDirectionMotor dropped, but could not disable enable pins: {:?}", err)
        };
    }
}

/// Represents a motor with an A and B pin. When moving forwards,
/// a PWM signal is sent through the A pin and the B pin is set to high,
/// vice versa for moving backwards. Note: If the dir_flip attribute is set to
/// true, this functionality is reversed
pub(crate) struct AbMotor<B: Board> {
    board: B,
    a_pin: i32,
    b_pin: i32,
    is_on: bool,
    pwm_pin: i32,
    motor_settings: MotorSettings,
}

impl<B> AbMotor<B>
where
    B: Board,
{
    pub(crate) fn new(
        a_pin: i32,
        b_pin: i32,
        motor_settings: MotorSettings,
        board: B,
    ) -> anyhow::Result<Self> {
        let pwm_freq = motor_settings.pwm_frequency as u64;
        let enable_high = motor_settings.enable_pin_high;
        let enable_low = motor_settings.enable_pin_low;
        let mut res = Self {
            board,
            a_pin,
            b_pin,
            is_on: false,
            pwm_pin: a_pin,
            motor_settings,
        };

        set_enable_pins(
            &mut res.board,
            enable_high,
            enable_low,
            true,
        )?;

        // we start with this because we want to reserve a timer and PWM channel early
        // for boards where these are a limited resource
        res.board.set_pwm_frequency(a_pin, pwm_freq)?;
        res.board.set_pwm_duty(a_pin, 0.0)?;
        Ok(res)
    }

    pub(crate) fn from_config(cfg: ConfigType, board: BoardType) -> anyhow::Result<MotorType> {
        let pins = cfg
            .get_attribute::<MotorPinsConfig>("pins")
            .or(Err(anyhow::anyhow!(
                "cannot build motor, could not find 'pins' attribute"
            )))?;
        let motor_settings = MotorSettings::from_configs(&cfg, &pins);
        let a_pin = pins
            .a
            .ok_or(anyhow::anyhow!("cannot build AbMotor, need 'a' pin"))?;
        let b_pin = pins
            .b
            .ok_or(anyhow::anyhow!("cannot build AbMotor, need 'b' pin"))?;
        Ok(Arc::new(Mutex::new(AbMotor::new(
            a_pin,
            b_pin,
            motor_settings,
            board,
        )?)))
    }
}

impl<B> Motor for AbMotor<B>
where
    B: Board,
{
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        if !(-1.0..=1.0).contains(&pct) {
            anyhow::bail!("power outside limit, must be between -1.0 and 1.0")
        }
        if pct.abs() <= 0.001 {
            return self.stop();
        }
        let (pwm_pin, high_pin) = if (pct >= 0.001) == self.motor_settings.dir_flip {
            (self.b_pin, self.a_pin)
        } else {
            (self.a_pin, self.b_pin)
        };
        if pwm_pin != self.pwm_pin {
            self.board
                .set_pwm_frequency(pwm_pin, self.motor_settings.pwm_frequency as u64)?;
            self.board.set_pwm_frequency(self.pwm_pin, 0)?;
        }
        self.pwm_pin = pwm_pin;
        self.board.set_gpio_pin_level(high_pin, true)?;
        self.board.set_pwm_duty(pwm_pin, pct)?;
        self.is_on = true;
        Ok(())
    }

    fn get_position(&mut self) -> anyhow::Result<i32> {
        anyhow::bail!("position reporting not supported without an encoder")
    }

    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        let (pwr, dur) = go_for_math(self.motor_settings.max_rpm, rpm, revolutions)?;
        self.set_power(pwr)?;
        if dur.is_some() {
            return Ok(dur);
        }
        Ok(None)
    }

    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: false,
        }
    }
}

impl<B> Status for AbMotor<B>
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

impl<B> Actuator for AbMotor<B>
where
    B: Board,
{
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        Ok(self.is_on)
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        set_enable_pins(
            &mut self.board,
            self.motor_settings.enable_pin_high,
            self.motor_settings.enable_pin_low,
            false,
        )?;
        self.board.set_pwm_duty(self.pwm_pin, 0.0)?;
        self.board.set_gpio_pin_level(self.a_pin, false)?;
        self.board.set_gpio_pin_level(self.b_pin, false)?;
        self.is_on = false;
        Ok(())
    }
}

impl<B: Board> Drop for AbMotor<B> 
{
    fn drop(&mut self) {
        if let Err(err) = set_enable_pins(
            &mut self.board,
            self.motor_settings.enable_pin_high,
            self.motor_settings.enable_pin_low,
            false,
        ) {
            log::error!("AbMotor dropped, but could not disable enable pins: {:?}", err)
        };
    }
}

fn set_enable_pins(
    board: &mut dyn Board,
    enable_high_pin: Option<i32>,
    enable_low_pin: Option<i32>,
    enable: bool,
) -> anyhow::Result<()> {
    let enable_high_result = match enable_high_pin {
        Some(pin) => board.set_gpio_pin_level(pin, enable),
        None => Ok(()),
    };
    let enable_low_result = match enable_low_pin {
        Some(pin) => board.set_gpio_pin_level(pin, !enable),
        None => Ok(()),
    };
    match (enable_high_result, enable_low_result) {
        (Err(err), Ok(_)) => Err(anyhow::anyhow!(
            "motor could not set enable high pin to {:?} due to {:?}",
            enable,
            err
        )),
        (Ok(_), Err(err)) => Err(anyhow::anyhow!(
            "motor could not set enable low pin to {:?} due to {:?}",
            !enable,
            err
        )),
        (Err(high_err), Err(low_err)) => Err(anyhow::anyhow!(
            "motor could not set enable high to {:?} due to {:?}, and set enable low to {:?} due to {:?}",
            enable,
            high_err,
            !enable,
            low_err
        )),
        _ => Ok(()),
    }
}
