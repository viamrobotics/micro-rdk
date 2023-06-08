#![allow(dead_code)]
use crate::common::status::Status;
use futures_lite::future;
use log::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::board::BoardType;
use super::config::{AttributeError, Component, ConfigType, Kind};
use super::math_utils::go_for_math;
use super::registry::ComponentRegistry;
use super::stop::Stoppable;
use async_io::Timer;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_motor("fake", &FakeMotor::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
}

pub trait Motor: Status + Stoppable {
    /// Sets the percentage of the motor's total power that should be employed.
    /// expressed a value between `-1.0` and `1.0` where negative values indicate a backwards
    /// direction and positive values a forward direction.
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()>;
    /// Reports the position of the robot's motor relative to its zero position.
    /// This method will return an error if position reporting is not supported.
    fn get_position(&mut self) -> anyhow::Result<i32>;
    /// Instructs the motor to turn at a specified speed, which is expressed in RPM,
    /// for a specified number of rotations relative to its starting position.
    /// This method will return an error if position reporting is not supported.
    /// If revolutions is 0, this will run the motor at rpm indefinitely.
    /// If revolutions != 0, this will block until the number of revolutions has been completed or another operation comes in.
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<()>;
}

pub(crate) type MotorType = Arc<Mutex<dyn Motor>>;

#[derive(Debug, Default)]
pub(crate) struct MotorPinsConfig {
    pub(crate) a: Option<i32>,
    pub(crate) b: Option<i32>,
    pub(crate) pwm: i32,
}

pub struct FakeMotor {
    pos: f64,
    power: f64,
    max_rpm: f64,
}

impl TryFrom<Kind> for MotorPinsConfig {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        let mut motor = MotorPinsConfig::default();
        match value {
            Kind::StructValueStatic(v) => {
                motor.pwm = v
                    .get("pwm")
                    .ok_or_else(|| AttributeError::KeyNotFound("pwm".to_string()))?
                    .try_into()?;
                if let Some(a) = v.get("a") {
                    motor.a = Some(a.try_into()?);
                } else {
                    motor.a = None;
                }
                if let Some(b) = v.get("b") {
                    motor.b = Some(b.try_into()?);
                } else {
                    motor.b = None;
                }
            }
            _ => return Err(AttributeError::ConversionImpossibleError),
        }
        Ok(motor)
    }
}

impl TryFrom<&Kind> for MotorPinsConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        let mut motor = MotorPinsConfig::default();
        match value {
            Kind::StructValueStatic(v) => {
                motor.pwm = v
                    .get("pwm")
                    .ok_or_else(|| AttributeError::KeyNotFound("pwm".to_string()))?
                    .try_into()?;
                motor.a = Some(
                    v.get("a")
                        .ok_or_else(|| AttributeError::KeyNotFound("a".to_string()))?
                        .try_into()?,
                );
                motor.b = Some(
                    v.get("b")
                        .ok_or_else(|| AttributeError::KeyNotFound("b".to_string()))?
                        .try_into()?,
                );
            }
            _ => return Err(AttributeError::ConversionImpossibleError),
        }
        Ok(motor)
    }
}

impl FakeMotor {
    pub fn new() -> Self {
        Self {
            pos: 10.0,
            power: 0.0,
            max_rpm: 0.0,
        }
    }
    pub(crate) fn from_config(cfg: ConfigType, _: Option<BoardType>) -> anyhow::Result<MotorType> {
        let mut motor = FakeMotor::default();
        match cfg {
            ConfigType::Static(cfg) => {
                motor.pos = cfg.get_attribute::<f64>("fake_position")?;

                motor.max_rpm = cfg.get_attribute::<f64>("max_rpm")?;
            }
        }

        Ok(Arc::new(Mutex::new(motor)))
    }
}
impl Default for FakeMotor {
    fn default() -> Self {
        Self::new()
    }
}

impl<L> Motor for Mutex<L>
where
    L: ?Sized + Motor,
{
    fn get_position(&mut self) -> anyhow::Result<i32> {
        self.get_mut().unwrap().get_position()
    }
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        self.get_mut().unwrap().set_power(pct)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<()> {
        self.get_mut().unwrap().go_for(rpm, revolutions)
    }
}

impl<A> Motor for Arc<Mutex<A>>
where
    A: ?Sized + Motor,
{
    fn get_position(&mut self) -> anyhow::Result<i32> {
        self.lock().unwrap().get_position()
    }
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        self.lock().unwrap().set_power(pct)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<()> {
        self.lock().unwrap().go_for(rpm, revolutions)
    }
}

impl Motor for FakeMotor {
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(self.pos as i32)
    }
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        debug!("setting power to {}", pct);
        self.power = pct;
        Ok(())
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<()> {
        // get_max_rpm
        let (pwr, dur) = go_for_math(self.max_rpm, rpm, revolutions).unwrap();
        if let Some(dur) = dur {
            self.set_power(pwr)?;
            future::block_on(async {
                Timer::after(dur).await;
            });
            self.stop()?;
        } else {
            self.set_power(pwr)?;
        }
        Ok(())
    }
}
impl Status for FakeMotor {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        bt.insert(
            "position".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(self.pos)),
            },
        );
        bt.insert(
            "position_reporting".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::BoolValue(true)),
            },
        );

        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

impl Stoppable for FakeMotor {
    fn stop(&mut self) -> anyhow::Result<()> {
        debug!("stopping motor");
        self.set_power(0.0)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::common::config::{Component, Kind, RobotConfigStatic, StaticComponentConfig};
    use crate::common::motor::{ConfigType, FakeMotor, MotorPinsConfig};
    #[test_log::test]
    fn test_motor_config() -> anyhow::Result<()> {
        #[allow(clippy::redundant_static_lifetimes, dead_code)]
        const STATIC_ROBOT_CONFIG: Option<RobotConfigStatic> = Some(RobotConfigStatic {
            components: Some(&[StaticComponentConfig {
                name: "motor",
                namespace: "rdk",
                r#type: "motor",
                model: "gpio",
                attributes: Some(phf::phf_map! {
                    "max_rpm" => Kind::NumberValue(10000f64),
                    "fake_position" => Kind::NumberValue(10f64),
                    "board" => Kind::StringValueStatic("board"),
                    "pins" => Kind::StructValueStatic(
                        phf::phf_map!{
                            "a" => Kind::StringValueStatic("11"),
                            "b" => Kind::StringValueStatic("12"),
                            "pwm" => Kind::StringValueStatic("13"),
                        }
                    )
                }),
            }]),
        });
        let val = STATIC_ROBOT_CONFIG.unwrap().components.unwrap()[0]
            .get_attribute::<MotorPinsConfig>("pins");
        assert!(&val.is_ok());

        let val = val.unwrap();

        assert!(val.a.is_some());
        assert_eq!(val.a.unwrap(), 11);
        assert!(val.b.is_some());
        assert_eq!(val.b.unwrap(), 12);
        assert_eq!(val.pwm, 13);

        let static_conf = ConfigType::Static(&STATIC_ROBOT_CONFIG.unwrap().components.unwrap()[0]);
        assert!(FakeMotor::from_config(static_conf, None).is_ok());

        Ok(())
    }
}
