#![allow(dead_code)]

#[cfg(feature = "builtin-components")]
use {
    crate::google,
    std::collections::HashMap,
    super::encoder::{
        Encoder, EncoderPositionType, EncoderType, COMPONENT_NAME as EncoderCompName,
    },
    super::math_utils::go_for_math,
    super::{
        config::ConfigType,
        registry::{ComponentRegistry, Dependency, ResourceKey},
        robot::Resource,
    }
};

use crate::common::status::Status;
use crate::proto::component::motor::v1::GetPropertiesResponse;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::actuator::{Actuator, ActuatorError};
use super::board::BoardError;
use super::config::{AttributeError, Kind};
use super::encoder::EncoderError;
use super::generic::DoCommand;
use super::math_utils::UtilsInvalidArg;

use thiserror::Error;

pub static COMPONENT_NAME: &str = "motor";

#[derive(Error, Debug)]
pub enum MotorError {
    #[error("invalid motor configuration")]
    InvalidMotorConfig,
    #[error(transparent)]
    EncoderError(#[from] EncoderError),
    #[error(transparent)]
    BoardError(#[from] BoardError),
    #[error("config error {0}")]
    ConfigError(&'static str),
    #[error("power must be between -1.0 and 1.0")]
    PowerSetError,
    #[error("missing encoder")]
    MissingEncoder,
    #[error(transparent)]
    InvalidArgument(#[from] UtilsInvalidArg),
    #[error(transparent)]
    ActuatorError(#[from] ActuatorError),
    #[error("unimplemented: {0}")]
    MotorMethodUnimplemented(&'static str),
}

#[cfg(feature = "builtin-components")]
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

pub struct MotorSupportedProperties {
    pub position_reporting: bool,
}

impl From<MotorSupportedProperties> for GetPropertiesResponse {
    fn from(value: MotorSupportedProperties) -> Self {
        GetPropertiesResponse {
            position_reporting: value.position_reporting,
        }
    }
}

pub trait Motor: Status + Actuator + DoCommand {
    /// Sets the percentage of the motor's total power that should be employed.
    /// expressed a value between `-1.0` and `1.0` where negative values indicate a backwards
    /// direction and positive values a forward direction.
    fn set_power(&mut self, pct: f64) -> Result<(), MotorError>;
    /// Reports the position of the robot's motor relative to its zero position.
    /// This method will return an error if position reporting is not supported.
    fn get_position(&mut self) -> Result<i32, MotorError>;
    /// Instructs the motor to turn at a specified speed, which is expressed in RPM,
    /// for a specified number of rotations relative to its starting position.
    /// This method will return an error if position reporting is not supported.
    /// If revolutions is 0, this will run the motor at rpm indefinitely.
    /// If revolutions != 0, this will block until the number of revolutions has been completed or another operation comes in.
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> Result<Option<Duration>, MotorError>;
    /// Returns an instance of MotorSupportedProperties indicating the optional properties
    /// supported by this motor
    fn get_properties(&mut self) -> MotorSupportedProperties;
}

pub type MotorType = Arc<Mutex<dyn Motor>>;

#[derive(Debug)]
pub enum MotorPinType {
    PwmAB,
    PwmDirection,
    AB,
}

#[derive(Debug, Default)]
pub struct MotorPinsConfig {
    pub(crate) a: Option<i32>,
    pub(crate) b: Option<i32>,
    pub(crate) dir: Option<i32>,
    pub(crate) pwm: Option<i32>,
}

impl MotorPinsConfig {
    pub fn detect_motor_type(&self) -> Result<MotorPinType, MotorError> {
        match self {
            x if (x.a.is_some() && x.b.is_some()) => match x.pwm {
                Some(_) => Ok(MotorPinType::PwmAB),
                None => Ok(MotorPinType::AB),
            },
            x if x.dir.is_some() => Ok(MotorPinType::PwmDirection),
            _ => Err(MotorError::InvalidMotorConfig),
        }
    }
}

#[cfg(feature = "builtin-components")]
#[derive(DoCommand)]
pub struct FakeMotor {
    pos: f64,
    power: f64,
    max_rpm: f64,
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
        let pwm = match value.get("pwm") {
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
        Ok(Self { a, b, dir, pwm })
    }
}

#[cfg(feature = "builtin-components")]
impl FakeMotor {
    pub fn new() -> Self {
        Self {
            pos: 10.0,
            power: 0.0,
            max_rpm: 100.0,
        }
    }
    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<MotorType, MotorError> {
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
#[cfg(feature = "builtin-components")]
impl Default for FakeMotor {
    fn default() -> Self {
        Self::new()
    }
}

impl<L> Motor for Mutex<L>
where
    L: ?Sized + Motor,
{
    fn get_position(&mut self) -> Result<i32, MotorError> {
        self.get_mut().unwrap().get_position()
    }
    fn set_power(&mut self, pct: f64) -> Result<(), MotorError> {
        self.get_mut().unwrap().set_power(pct)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> Result<Option<Duration>, MotorError> {
        self.get_mut().unwrap().go_for(rpm, revolutions)
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        self.get_mut().unwrap().get_properties()
    }
}

impl<A> Motor for Arc<Mutex<A>>
where
    A: ?Sized + Motor,
{
    fn get_position(&mut self) -> Result<i32, MotorError> {
        self.lock().unwrap().get_position()
    }
    fn set_power(&mut self, pct: f64) -> Result<(), MotorError> {
        self.lock().unwrap().set_power(pct)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> Result<Option<Duration>, MotorError> {
        self.lock().unwrap().go_for(rpm, revolutions)
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        self.lock().unwrap().get_properties()
    }
}

#[cfg(feature = "builtin-components")]
impl Motor for FakeMotor {
    fn get_position(&mut self) -> Result<i32, MotorError> {
        Ok(self.pos as i32)
    }
    fn set_power(&mut self, pct: f64) -> Result<(), MotorError> {
        log::debug!("setting power to {}", pct);
        self.power = pct;
        Ok(())
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> Result<Option<Duration>, MotorError> {
        // get_max_rpm
        let (pwr, dur) = go_for_math(self.max_rpm, rpm, revolutions)?;
        self.set_power(pwr)?;
        Ok(dur)
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: true,
        }
    }
}

#[cfg(feature = "builtin-components")]
impl Status for FakeMotor {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut hm = HashMap::new();
        hm.insert(
            "position".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(self.pos)),
            },
        );
        hm.insert(
            "position_reporting".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::BoolValue(true)),
            },
        );

        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}

#[cfg(feature = "builtin-components")]
impl Actuator for FakeMotor {
    fn stop(&mut self) -> Result<(), ActuatorError> {
        log::debug!("stopping motor");
        self.set_power(0.0).map_err(|_| ActuatorError::CouldntStop)
    }
    fn is_moving(&mut self) -> Result<bool, ActuatorError> {
        Ok(self.power > 0.0)
    }
}

#[cfg(feature = "builtin-components")]
#[derive(DoCommand)]
pub struct FakeMotorWithDependency {
    encoder: Option<EncoderType>,
    power: f64,
}

#[cfg(feature = "builtin-components")]
impl FakeMotorWithDependency {
    pub fn new(encoder: Option<EncoderType>) -> Self {
        Self {
            encoder,
            power: 0.0,
        }
    }

    pub(crate) fn dependencies_from_config(cfg: ConfigType) -> Vec<ResourceKey> {
        let mut r_keys = Vec::new();
        log::info!("getting deps");
        if let Ok(enc_name) = cfg.get_attribute::<String>("encoder") {
            let r_key = ResourceKey(EncoderCompName, enc_name);
            r_keys.push(r_key)
        }
        r_keys
    }

    pub(crate) fn from_config(
        _: ConfigType,
        deps: Vec<Dependency>,
    ) -> Result<MotorType, MotorError> {
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
        Ok(Arc::new(Mutex::new(Self::new(enc))))
    }
}

#[cfg(feature = "builtin-components")]
impl Motor for FakeMotorWithDependency {
    fn get_position(&mut self) -> Result<i32, MotorError> {
        match &self.encoder {
            Some(enc) => Ok(enc.get_position(EncoderPositionType::DEGREES)?.value as i32),
            None => Ok(0),
        }
    }
    fn set_power(&mut self, pct: f64) -> Result<(), MotorError> {
        log::debug!("setting power to {}", pct);
        self.power = pct;
        Ok(())
    }
    fn go_for(&mut self, _: f64, _: f64) -> Result<Option<Duration>, MotorError> {
        Err(MotorError::MotorMethodUnimplemented("go_for"))
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: true,
        }
    }
}

#[cfg(feature = "builtin-components")]
impl Status for FakeMotorWithDependency {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let hm = HashMap::new();
        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}

#[cfg(feature = "builtin-components")]
impl Actuator for FakeMotorWithDependency {
    fn stop(&mut self) -> Result<(), ActuatorError> {
        self.power = 0.0;
        Ok(())
    }
    fn is_moving(&mut self) -> Result<bool, ActuatorError> {
        Ok(self.power > 0.0)
    }
}

#[cfg(feature = "builtin-components")]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::config::{Component, DynamicComponentConfig, Kind};
    use crate::common::motor::{ConfigType, FakeMotor, MotorPinType, MotorPinsConfig};
    #[test_log::test]
    fn test_motor_config() -> anyhow::Result<()> {
        let robot_config: [Option<DynamicComponentConfig>; 1] = [Some(DynamicComponentConfig {
            name: "motor".to_owned(),
            namespace: "rdk".to_owned(),
            r#type: "motor".to_owned(),
            model: "gpio".to_owned(),
            attributes: Some(HashMap::from([
                ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                ("board".to_owned(), Kind::StringValue("board".to_owned())),
                (
                    "pins".to_owned(),
                    Kind::StructValue(HashMap::from([
                        ("a".to_owned(), Kind::StringValue("11".to_owned())),
                        ("b".to_owned(), Kind::StringValue("12".to_owned())),
                        ("pwm".to_owned(), Kind::StringValue("13".to_owned())),
                        ("dir".to_owned(), Kind::StringValue("14".to_owned())),
                    ])),
                ),
            ])),
        })];

        let val = robot_config[0]
            .as_ref()
            .unwrap()
            .get_attribute::<MotorPinsConfig>("pins");
        assert!(&val.is_ok());

        let val = val.unwrap();

        assert!(val.a.is_some());
        assert_eq!(val.a.unwrap(), 11);
        assert!(val.b.is_some());
        assert_eq!(val.b.unwrap(), 12);
        assert!(val.pwm.is_some());
        assert_eq!(val.pwm.unwrap(), 13);
        assert!(val.dir.is_some());
        assert_eq!(val.dir.unwrap(), 14);

        let dyn_conf = ConfigType::Dynamic(robot_config[0].as_ref().unwrap());
        assert!(FakeMotor::from_config(dyn_conf, Vec::new()).is_ok());

        Ok(())
    }

    #[test_log::test]
    fn test_detect_motor_type_from_cfg() {
        let robot_config: [Option<DynamicComponentConfig>; 4] = [
            Some(DynamicComponentConfig {
                name: "motor".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                    ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([
                            ("a".to_owned(), Kind::StringValue("11".to_owned())),
                            ("b".to_owned(), Kind::StringValue("12".to_owned())),
                            ("pwm".to_owned(), Kind::StringValue("13".to_owned())),
                        ])),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "motor".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                    ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([
                            ("dir".to_owned(), Kind::StringValue("11".to_owned())),
                            ("pwm".to_owned(), Kind::StringValue("13".to_owned())),
                        ])),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "motor2".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                    ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([(
                            "pwm".to_owned(),
                            Kind::StringValue("13".to_owned()),
                        )])),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "motor3".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                    ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([
                            ("a".to_owned(), Kind::StringValue("11".to_owned())),
                            ("b".to_owned(), Kind::StringValue("13".to_owned())),
                        ])),
                    ),
                ])),
            }),
        ];

        let dyn_cfg = ConfigType::Dynamic(robot_config[0].as_ref().unwrap());
        let pin_cfg_result = dyn_cfg.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result.is_ok());
        let motor_type = pin_cfg_result.unwrap().detect_motor_type();
        assert!(motor_type.is_ok());
        assert!(matches!(motor_type.unwrap(), MotorPinType::PwmAB));

        let dyn_cfg_2 = ConfigType::Dynamic(robot_config[1].as_ref().unwrap());
        let pin_cfg_result_2 = dyn_cfg_2.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result_2.is_ok());
        let motor_type_2 = pin_cfg_result_2.unwrap().detect_motor_type();
        assert!(motor_type_2.is_ok());
        assert!(matches!(motor_type_2.unwrap(), MotorPinType::PwmDirection));

        let dyn_cfg_3 = ConfigType::Dynamic(robot_config[2].as_ref().unwrap());
        let pin_cfg_result_3 = dyn_cfg_3.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result_3.is_ok());
        let motor_type_3 = pin_cfg_result_3.unwrap().detect_motor_type();
        assert!(motor_type_3.is_err());

        let dyn_cfg_4 = ConfigType::Dynamic(robot_config[3].as_ref().unwrap());
        let pin_cfg_result_4 = dyn_cfg_4.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result_4.is_ok());
        let motor_type_4 = pin_cfg_result_4.unwrap().detect_motor_type();
        assert!(motor_type_4.is_ok());
        assert!(matches!(motor_type_4.unwrap(), MotorPinType::AB));
    }
}
