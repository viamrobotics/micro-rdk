use super::actuator::{Actuator, ActuatorError};
use super::base::{Base, BaseError, BaseType, COMPONENT_NAME as BaseCompName};
use super::config::ConfigType;
use super::motor::{Motor, MotorType, COMPONENT_NAME as MotorCompName};
use super::registry::{ComponentRegistry, Dependency, ResourceKey};
use super::robot::Resource;
use super::status::Status;
use crate::google;
use crate::proto::common::v1::Vector3;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_base(
            "two_wheeled_base",
            &WheeledBase::<MotorType, MotorType>::from_config,
        )
        .is_err()
    {
        log::error!("two_wheeled_base model is already registered")
    }
    if registry
        .register_dependency_getter(
            BaseCompName,
            "two_wheeled_base",
            &WheeledBase::<MotorType, MotorType>::dependencies_from_config,
        )
        .is_err()
    {
        log::error!("failed to register dependency getter for two_wheeled_base model")
    }
}

#[derive(DoCommand)]
pub struct WheeledBase<ML, MR> {
    motor_right: MR,
    motor_left: ML,
}

impl<ML, MR> WheeledBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    pub fn new(motor_left: ML, motor_right: MR) -> Self {
        WheeledBase {
            motor_right,
            motor_left,
        }
    }
    #[allow(clippy::only_used_in_recursion)]
    fn differential_drive(&self, forward: f64, left: f64) -> (f64, f64) {
        if forward < 0.0 {
            let (r, l) = self.differential_drive(-forward, left);
            return (-r, -l);
        }
        let r = forward.hypot(left);
        let mut t = left.atan2(forward);
        t += std::f64::consts::FRAC_PI_4;
        let l = (r * t.cos()) * std::f64::consts::SQRT_2;
        let r = (r * t.sin()) * std::f64::consts::SQRT_2;
        (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0))
    }

    pub(crate) fn from_config(
        cfg: ConfigType,
        deps: Vec<Dependency>,
    ) -> Result<BaseType, BaseError> {
        let l_motor_name = cfg.get_attribute::<String>("left")?;
        let r_motor_name = cfg.get_attribute::<String>("right")?;
        let mut l_motor: Option<MotorType> = None;
        let mut r_motor: Option<MotorType> = None;
        for Dependency(key, res) in deps {
            if let Resource::Motor(found_motor) = res {
                match key.1 {
                    x if x == l_motor_name => {
                        l_motor = Some(found_motor.clone());
                    }
                    x if x == r_motor_name => {
                        r_motor = Some(found_motor.clone());
                    }
                    _ => {}
                };
            }
        }
        if let Some(l_motor) = l_motor {
            if let Some(r_motor) = r_motor {
                Ok(Arc::new(Mutex::new(WheeledBase::new(r_motor, l_motor))))
            } else {
                return Err(BaseError::BaseConfigError("right motor couldn't be found"));
            }
        } else {
            return Err(BaseError::BaseConfigError("left motor couldn't be found"));
        }
    }

    pub(crate) fn dependencies_from_config(cfg: ConfigType) -> Vec<ResourceKey> {
        let mut r_keys = Vec::new();
        if let Ok(l_motor_name) = cfg.get_attribute::<String>("left") {
            let r_key = ResourceKey(MotorCompName, l_motor_name);
            r_keys.push(r_key)
        }
        if let Ok(r_motor_name) = cfg.get_attribute::<String>("right") {
            let r_key = ResourceKey(MotorCompName, r_motor_name);
            r_keys.push(r_key)
        }
        r_keys
    }
}
impl<ML, MR> Status for WheeledBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut hm = HashMap::new();
        hm.insert(
            "is_moving".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::BoolValue(false)),
            },
        );
        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}

impl<ML, MR> Actuator for WheeledBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    fn is_moving(&mut self) -> Result<bool, ActuatorError> {
        Ok(self.motor_left.is_moving()? || self.motor_right.is_moving()?)
    }
    fn stop(&mut self) -> Result<(), ActuatorError> {
        self.motor_left.stop()?;
        self.motor_right.stop()?;
        Ok(())
    }
}

impl<ML, MR> Base for WheeledBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> Result<(), BaseError> {
        let (l, r) = self.differential_drive(lin.y, ang.z);
        self.motor_left.set_power(l)?;
        self.motor_right.set_power(r)?;
        Ok(())
    }
}
