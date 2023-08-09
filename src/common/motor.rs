#![allow(dead_code)]
use crate::common::status::Status;
use log::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::config::{AttributeError, ConfigType, Kind};
use super::encoder::{
    Encoder, EncoderPositionType, EncoderType, COMPONENT_NAME as EncoderCompName,
};
use super::math_utils::go_for_math;
use super::registry::{ComponentRegistry, Dependency, ResourceKey};
use super::robot::Resource;
use super::stop::Stoppable;

pub static COMPONENT_NAME: &str = "motor";

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_motor("fake", &FakeMotor::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
    if registry
        .register_motor("fake_with_dep", &FakeMotorWithDependency::from_config)
        .is_err()
    {
        log::error!("fake_with_dep type is already registered");
    }
    if registry
        .register_dependency_getter(
            COMPONENT_NAME,
            "fake_with_dep",
            &FakeMotorWithDependency::dependencies_from_config,
        )
        .is_err()
    {
        log::error!("fake_with_dep type dependency function is already registered");
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
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>>;
}

pub type MotorType = Arc<Mutex<dyn Motor>>;

#[derive(Debug)]
pub enum MotorPinType {
    PwmAB,
    PwmDirection,
}

#[derive(Debug, Default)]
pub struct MotorPinsConfig {
    pub(crate) a: Option<i32>,
    pub(crate) b: Option<i32>,
    pub(crate) dir: Option<i32>,
    pub(crate) pwm: i32,
}

impl MotorPinsConfig {
    pub fn detect_motor_type(&self) -> anyhow::Result<MotorPinType> {
        match self {
            x if (x.a.is_some() && x.b.is_some()) => Ok(MotorPinType::PwmAB),
            x if x.dir.is_some() => Ok(MotorPinType::PwmDirection),
            _ => Err(anyhow::anyhow!("invalid pin parameters for motor")),
        }
    }
}

pub struct FakeMotor {
    pos: f64,
    power: f64,
    max_rpm: f64,
}

impl TryFrom<Kind> for MotorPinsConfig {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        let a = match value.get("a") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        let b = match value.get("b") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        let dir = match value.get("dir") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        let pwm = value
            .get("pwm")?
            .ok_or_else(|| AttributeError::KeyNotFound("pwm".to_string()))?
            .try_into()?;
        Ok(Self { a, b, dir, pwm })
    }
}

impl TryFrom<&Kind> for MotorPinsConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        let a = match value.get("a") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        let b = match value.get("b") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        let dir = match value.get("dir") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        let pwm = value
            .get("pwm")?
            .ok_or_else(|| AttributeError::KeyNotFound("pwm".to_string()))?
            .try_into()?;
        Ok(Self { a, b, dir, pwm })
    }
}

impl FakeMotor {
    pub fn new() -> Self {
        Self {
            pos: 10.0,
            power: 0.0,
            max_rpm: 100.0,
        }
    }
    pub(crate) fn from_config(cfg: ConfigType, _: Vec<Dependency>) -> anyhow::Result<MotorType> {
        let mut motor = FakeMotor::default();
        if let Ok(pos) = cfg.get_attribute::<f64>("fake_position") {
            motor.pos = pos
        }
        if let Ok(max_rpm) = cfg.get_attribute::<f64>("max_rpm") {
            motor.max_rpm = max_rpm
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
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
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
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
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
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        // get_max_rpm
        let (pwr, dur) = go_for_math(self.max_rpm, rpm, revolutions)?;
        self.set_power(pwr)?;
        Ok(dur)
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

pub struct FakeMotorWithDependency {
    encoder: Option<EncoderType>,
}

impl FakeMotorWithDependency {
    pub fn new(encoder: EncoderType) -> Self {
        Self {
            encoder: Some(encoder),
        }
    }

    pub(crate) fn dependencies_from_config(cfg: ConfigType) -> Vec<ResourceKey> {
        let mut r_keys = Vec::new();
        if let Ok(enc_name) = cfg.get_attribute::<String>("encoder") {
            let r_key = ResourceKey(EncoderCompName, enc_name);
            r_keys.push(r_key)
        }
        r_keys
    }

    pub(crate) fn from_config(_: ConfigType, deps: Vec<Dependency>) -> anyhow::Result<MotorType> {
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
        Ok(Arc::new(Mutex::new(Self { encoder: enc })))
    }
}

impl Motor for FakeMotorWithDependency {
    fn get_position(&mut self) -> anyhow::Result<i32> {
        match &self.encoder {
            Some(enc) => Ok(enc.get_position(EncoderPositionType::DEGREES)?.value as i32),
            None => Ok(0),
        }
    }
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        debug!("setting power to {}", pct);
        Ok(())
    }
    fn go_for(&mut self, _: f64, _: f64) -> anyhow::Result<Option<Duration>> {
        anyhow::bail!("go_for unimplemented")
    }
}

impl Status for FakeMotorWithDependency {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let bt = BTreeMap::new();
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

impl Stoppable for FakeMotorWithDependency {
    fn stop(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::common::config::{Component, Kind, RobotConfigStatic, StaticComponentConfig};
    use crate::common::motor::{ConfigType, FakeMotor, MotorPinType, MotorPinsConfig};
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
                            "dir"=> Kind::StringValueStatic("14")
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
        assert!(val.dir.is_some());
        assert_eq!(val.dir.unwrap(), 14);

        let static_conf = ConfigType::Static(&STATIC_ROBOT_CONFIG.unwrap().components.unwrap()[0]);
        assert!(FakeMotor::from_config(static_conf, Vec::new()).is_ok());

        Ok(())
    }

    #[test_log::test]
    fn test_detect_motor_type_from_cfg() {
        #[allow(clippy::redundant_static_lifetimes, dead_code)]
        const STATIC_ROBOT_CONFIG: Option<RobotConfigStatic> = Some(RobotConfigStatic {
            components: Some(&[
                StaticComponentConfig {
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
                },
                StaticComponentConfig {
                    name: "motor2",
                    namespace: "rdk",
                    r#type: "motor",
                    model: "gpio",
                    attributes: Some(phf::phf_map! {
                        "max_rpm" => Kind::NumberValue(10000f64),
                        "fake_position" => Kind::NumberValue(10f64),
                        "board" => Kind::StringValueStatic("board"),
                        "pins" => Kind::StructValueStatic(
                            phf::phf_map!{
                                "dir" => Kind::StringValueStatic("11"),
                                "pwm" => Kind::StringValueStatic("13"),
                            }
                        )
                    }),
                },
                StaticComponentConfig {
                    name: "bad_motor",
                    namespace: "rdk",
                    r#type: "motor",
                    model: "gpio",
                    attributes: Some(phf::phf_map! {
                        "max_rpm" => Kind::NumberValue(10000f64),
                        "fake_position" => Kind::NumberValue(10f64),
                        "board" => Kind::StringValueStatic("board"),
                        "pins" => Kind::StructValueStatic(
                            phf::phf_map!{
                                "pwm" => Kind::StringValueStatic("13"),
                            }
                        )
                    }),
                },
            ]),
        });

        let static_cfg = ConfigType::Static(&STATIC_ROBOT_CONFIG.unwrap().components.unwrap()[0]);
        let pin_cfg_result = static_cfg.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result.is_ok());
        let motor_type = pin_cfg_result.unwrap().detect_motor_type();
        assert!(motor_type.is_ok());
        assert!(matches!(motor_type.unwrap(), MotorPinType::PwmAB));

        let static_cfg_2 = ConfigType::Static(&STATIC_ROBOT_CONFIG.unwrap().components.unwrap()[1]);
        let pin_cfg_result_2 = static_cfg_2.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result_2.is_ok());
        let motor_type_2 = pin_cfg_result_2.unwrap().detect_motor_type();
        assert!(motor_type_2.is_ok());
        assert!(matches!(motor_type_2.unwrap(), MotorPinType::PwmDirection));

        let static_cfg_3 = ConfigType::Static(&STATIC_ROBOT_CONFIG.unwrap().components.unwrap()[2]);
        let pin_cfg_result_3 = static_cfg_3.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result_3.is_ok());
        let motor_type_3 = pin_cfg_result_3.unwrap().detect_motor_type();
        assert!(motor_type_3.is_err());
    }
}
