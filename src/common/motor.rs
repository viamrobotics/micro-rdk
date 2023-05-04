#![allow(dead_code)]
use crate::common::status::Status;
use log::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::board::BoardType;
use super::config::{AttributeError, Component, ConfigType, Kind};
use super::registry::ComponentRegistry;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_motor("fake", &FakeMotor::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
}

pub trait Position {
    fn position(&self) -> anyhow::Result<i32> {
        Ok(0)
    }
}

pub trait Motor: Status {
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()>;
    fn get_position(&mut self) -> anyhow::Result<i32>;
}

pub(crate) type MotorType = Arc<Mutex<dyn Motor>>;

#[derive(Debug, Default)]
pub(crate) struct MotorConfig {
    pub(crate) a: Option<i32>,
    pub(crate) b: Option<i32>,
    pub(crate) pwm: i32,
}

pub struct FakeMotor {
    pos: f64,
    power: f64,
}

impl TryFrom<Kind> for MotorConfig {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        let mut motor = MotorConfig::default();
        match value {
            Kind::StructValueStatic(v) => {
                if !v.contains_key("pwm") {
                    return Err(AttributeError::KeyNotFound);
                }
                motor.pwm = v.get("pwm").unwrap().try_into()?;
                if v.contains_key("a") {
                    motor.a = Some(v.get("a").unwrap().try_into()?);
                }
                if v.contains_key("b") {
                    motor.b = Some(v.get("b").unwrap().try_into()?);
                }
            }
            _ => return Err(AttributeError::ConversionImpossibleError),
        }
        Ok(motor)
    }
}

impl TryFrom<&Kind> for MotorConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        let mut motor = MotorConfig::default();
        match value {
            Kind::StructValueStatic(v) => {
                if !v.contains_key("pwm") {
                    return Err(AttributeError::KeyNotFound);
                }
                motor.pwm = v.get("pwm").unwrap().try_into()?;
                if v.contains_key("a") {
                    motor.a = Some(v.get("a").unwrap().try_into()?);
                }
                if v.contains_key("b") {
                    motor.b = Some(v.get("b").unwrap().try_into()?);
                }
            }
            _ => return Err(AttributeError::ConversionImpossibleError),
        }
        Ok(motor)
    }
}

impl FakeMotor {
    pub fn new() -> Self {
        FakeMotor {
            pos: 10.0,
            power: 0.0,
        }
    }
    pub(crate) fn from_config(cfg: ConfigType, _: Option<BoardType>) -> anyhow::Result<MotorType> {
        match cfg {
            ConfigType::Static(cfg) => {
                if let Ok(pos) = cfg.get_attribute::<f64>("fake_position") {
                    return Ok(Arc::new(Mutex::new(FakeMotor { pos, power: 0.0 })));
                }
            }
        };

        Ok(Arc::new(Mutex::new(FakeMotor::new())))
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
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        self.get_mut().unwrap().set_power(pct)
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        self.get_mut().unwrap().get_position()
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
}

impl Motor for FakeMotor {
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        info!("setting power to {}", pct);
        self.power = pct;
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(self.pos as i32)
    }
}
impl Status for FakeMotor {
    fn get_status(&mut self) -> anyhow::Result<Option<prost_types::Struct>> {
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

#[cfg(test)]
mod tests {
    use crate::common::config::{Component, Kind, RobotConfigStatic, StaticComponentConfig};
    use crate::common::motor::MotorConfig;
    #[test_log::test]
    fn test_motor_config() -> anyhow::Result<()> {
        #[allow(clippy::redundant_static_lifetimes, dead_code)]
        const STATIC_ROBOT_CONFIG: Option<RobotConfigStatic> = Some(RobotConfigStatic {
            components: Some(&[StaticComponentConfig {
                name: "motor",
                namespace: "rdk",
                r#type: "motor",
                model: "gpio",
                attributes: Some(
                    phf::phf_map! {"max_rpm" => Kind::NumberValue(10000f64),"board" => Kind::StringValueStatic("board"),"pins" => Kind::StructValueStatic(phf::phf_map!{"a" => Kind::StringValueStatic("11"),"pwm" => Kind::StringValueStatic("13"),"b" => Kind::StringValueStatic("12")})},
                ),
            }]),
        });
        let val = STATIC_ROBOT_CONFIG.unwrap().components.unwrap()[0]
            .get_attribute::<MotorConfig>("pins");
        assert!(&val.is_ok());

        let val = val.unwrap();

        assert!(val.a.is_some());
        assert_eq!(val.a.unwrap(), 11);
        assert!(val.b.is_some());
        assert_eq!(val.b.unwrap(), 12);
        assert_eq!(val.pwm, 13);
        Ok(())
    }
}
