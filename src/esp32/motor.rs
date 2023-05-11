#![allow(dead_code)]
use esp_idf_hal::gpio::{AnyOutputPin, Output, PinDriver};
use esp_idf_hal::ledc::config::TimerConfig;
use esp_idf_hal::ledc::{LedcDriver, LedcTimerDriver, CHANNEL0, CHANNEL1, CHANNEL2};

use super::pin::PinExt;
use crate::common::board::BoardType;
use crate::common::config::{Component, ConfigType};
use crate::common::encoder::{Encoder, EncoderPositionType};
use crate::common::motor::{Motor, MotorConfig, MotorType};
use crate::common::registry::ComponentRegistry;
use crate::common::status::Status;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use embedded_hal::digital::v2::OutputPin;
use embedded_hal::PwmPin;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_motor(
            "gpio",
            &ABMotorEsp32::<
                PinDriver<'_, AnyOutputPin, Output>,
                PinDriver<'_, AnyOutputPin, Output>,
                LedcDriver<'_>,
            >::from_config,
        )
        .is_err()
    {
        log::error!("gpio model is already registered")
    }
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
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        self.motor.set_power(pct)
    }

    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(self
            .enc
            .get_position(EncoderPositionType::UNSPECIFIED)?
            .value as i32)
    }
}

impl<M, Enc> Status for EncodedMotor<M, Enc>
where
    M: Motor,
    Enc: Encoder,
{
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let pos = self
            .enc
            .get_position(EncoderPositionType::UNSPECIFIED)?
            .value as f64;
        bt.insert(
            "position".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

// Represents a motor using a A, B, and PWM pins
pub struct ABMotorEsp32<A, B, PWM> {
    a: A,
    b: B,
    pwm: PWM,
}

impl<A, B, PWM> ABMotorEsp32<A, B, PWM>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
{
    pub fn new(a: A, b: B, pwm: PWM) -> Self {
        ABMotorEsp32 { a, b, pwm }
    }

    pub(crate) fn from_config(cfg: ConfigType, _: Option<BoardType>) -> anyhow::Result<MotorType> {
        match cfg {
            ConfigType::Static(cfg) => {
                if let Ok(pins) = cfg.get_attribute::<MotorConfig>("pins") {
                    if pins.a.is_some() && pins.b.is_some() {
                        use esp_idf_hal::units::FromValueType;
                        let pwm_tconf = TimerConfig::default().frequency(10.kHz().into());
                        let timer = LedcTimerDriver::new(
                            unsafe { esp_idf_hal::ledc::TIMER0::new() },
                            &pwm_tconf,
                        )?;
                        let pwm_pin = unsafe { AnyOutputPin::new(pins.pwm) };
                        let chan = PWMCHANNELS.lock().unwrap().take_next_channel()?;
                        let chan = match chan {
                            PwmChannel::C0(c) => LedcDriver::new(c, timer, pwm_pin)?,
                            PwmChannel::C1(c) => LedcDriver::new(c, timer, pwm_pin)?,
                            PwmChannel::C2(c) => LedcDriver::new(c, timer, pwm_pin)?,
                        };
                        return Ok(Arc::new(Mutex::new(ABMotorEsp32::new(
                            PinDriver::output(unsafe { AnyOutputPin::new(pins.a.unwrap()) })?,
                            PinDriver::output(unsafe { AnyOutputPin::new(pins.b.unwrap()) })?,
                            chan,
                        ))));
                    }
                }
            }
        }
        Err(anyhow::anyhow!("cannot build motor"))
    }
}

impl<A, B, PWM> Motor for ABMotorEsp32<A, B, PWM>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
{
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        if !(-1.0..=1.0).contains(&pct) {
            anyhow::bail!("power outside limit")
        }
        let max_duty = self.pwm.get_max_duty();
        if pct < 0.0 {
            self.a
                .set_high()
                .map_err(|_| anyhow::anyhow!("error setting A pin"))?;
            self.b
                .set_low()
                .map_err(|_| anyhow::anyhow!("error setting B pin"))?;
        } else {
            self.a
                .set_low()
                .map_err(|_| anyhow::anyhow!("error setting A pin"))?;
            self.b
                .set_high()
                .map_err(|_| anyhow::anyhow!("error setting B pin"))?;
        }
        self.pwm
            .set_duty(((max_duty as f64) * pct.abs()).floor() as u32);
        println!("set to {:?} pct", pct);
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(0)
    }
}

impl<A, B, PWM> Status for ABMotorEsp32<A, B, PWM>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
{
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let pos = 0.0;
        bt.insert(
            "position".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

// Represents a motor using a direction pin and a PWM pin
pub struct PWMDirMotorEsp32<DIR, PWM> {
    dir: DIR,
    pwm: PWM,
    dir_flip: bool,
}

impl<DIR, PWM> PWMDirMotorEsp32<DIR, PWM>
where
    DIR: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
{
    pub fn new(dir: DIR, pwm: PWM, dir_flip: bool) -> Self {
        Self { dir, pwm, dir_flip }
    }
}

impl<DIR, PWM> Motor for PWMDirMotorEsp32<DIR, PWM>
where
    DIR: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
{
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        if !(-1.0..=1.0).contains(&pct) {
            anyhow::bail!("power outside limit")
        }
        let max_duty = self.pwm.get_max_duty();
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
        self.pwm
            .set_duty(((max_duty as f64) * pct.abs()).floor() as u32);
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(0)
    }
}

impl<DIR, PWM> Status for PWMDirMotorEsp32<DIR, PWM>
where
    DIR: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
{
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let pos = 0.0;
        bt.insert(
            "position".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

/// Below is a first attempt as an approach to runtime configuration, the problem raise with runtime configuration is
/// enforcing single instance of any peripheral at any point in the program. For example say you have a Motor that uses pins 33,34 and 35 and
/// a AnalogReader that uses pin 35. This situation is wrong since two objects own pin 35. In embedded rust this is avoided by having any hardware
/// peripherals be singleton and leveraging the borrow checker so that single ownership rules are enforced. When dealing with runtime configuration,
/// the borrow checker cannot help us. We can however follow the singleton approach and wrap peripherals into options that will be 'taken out' when
/// something needs an hardware component. Following is an implementation of the proposed approach, a significant limitation is that the hardware can
/// only be taken once and can never be returned.
enum PwmChannel {
    C0(CHANNEL0),
    C1(CHANNEL1),
    C2(CHANNEL2),
}
struct PwmChannels {
    channel0: Option<CHANNEL0>,
    channel1: Option<CHANNEL1>,
    channel2: Option<CHANNEL2>,
}

impl PwmChannels {
    fn take_channel(&mut self, n: i32) -> anyhow::Result<PwmChannel> {
        match n {
            0 => {
                if self.channel0.is_some() {
                    let chan = self.channel0.take().unwrap();
                    return Ok(PwmChannel::C0(chan));
                }
                Err(anyhow::anyhow!("channel 0 already taken"))
            }
            1 => {
                if self.channel1.is_some() {
                    let chan = self.channel1.take().unwrap();
                    return Ok(PwmChannel::C1(chan));
                }
                Err(anyhow::anyhow!("channel 1 already taken"))
            }
            2 => {
                if self.channel2.is_some() {
                    let chan = self.channel2.take().unwrap();
                    return Ok(PwmChannel::C2(chan));
                }
                Err(anyhow::anyhow!("channel 2 already taken"))
            }
            _ => Err(anyhow::anyhow!("no channel {}", n)),
        }
    }
    fn take_next_channel(&mut self) -> anyhow::Result<PwmChannel> {
        for i in 0..2 {
            let ret = self.take_channel(i);
            if ret.is_ok() {
                return ret;
            }
        }
        Err(anyhow::anyhow!("no more channel available"))
    }
}

lazy_static::lazy_static! {
    static ref PWMCHANNELS: Mutex<PwmChannels> = Mutex::new(PwmChannels {
        channel0: Some(unsafe { CHANNEL0::new() }),
        channel1: Some(unsafe { CHANNEL1::new() }),
        channel2: Some(unsafe { CHANNEL2::new() }),
    });
}
