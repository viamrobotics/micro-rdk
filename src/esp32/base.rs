#![allow(dead_code)]
use crate::common::base::{Base, BaseType, COMPONENT_NAME as BaseCompName};
use crate::common::config::ConfigType;
use crate::common::motor::{Motor, MotorType, COMPONENT_NAME as MotorCompName};
use crate::common::registry::{ComponentRegistry, Dependency, ResourceKey};
use crate::common::robot::Resource;
use crate::common::status::Status;
use crate::common::stop::Stoppable;
use crate::proto::common::v1::Vector3;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_base(
            "esp32_wheeled_base",
            &Esp32WheelBase::<MotorType, MotorType>::from_config,
        )
        .is_err()
    {
        log::error!("esp32_wheeled_base model is already registered")
    }
    if registry
        .register_dependency_getter(
            BaseCompName,
            "esp32_wheeled_base",
            &Esp32WheelBase::<MotorType, MotorType>::dependencies_from_config,
        )
        .is_err()
    {
        log::error!("failed to register dependency getter for esp32_wheeled_base model")
    }
}

pub struct Esp32WheelBase<ML, MR> {
    motor_right: MR,
    motor_left: ML,
}

impl<ML, MR> Esp32WheelBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    pub fn new(motor_left: ML, motor_right: MR) -> Self {
        Esp32WheelBase {
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

    pub(crate) fn from_config(cfg: ConfigType, deps: Vec<Dependency>) -> anyhow::Result<BaseType> {
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
                Ok(Arc::new(Mutex::new(Esp32WheelBase::new(r_motor, l_motor))))
            } else {
                anyhow::bail!(
                    "right motor for base not found in dependencies, looking for motor named {:?}",
                    r_motor_name
                );
            }
        } else {
            anyhow::bail!(
                "left motor for base not found in dependencies, looking for motor named {:?}",
                l_motor_name
            );
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
impl<ML, MR> Status for Esp32WheelBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        bt.insert(
            "is_moving".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::BoolValue(false)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

impl<ML, MR> Stoppable for Esp32WheelBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    fn stop(&mut self) -> anyhow::Result<()> {
        self.motor_left.stop()?;
        self.motor_right.stop()?;
        Ok(())
    }
}

impl<ML, MR> Base for Esp32WheelBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        let (l, r) = self.differential_drive(lin.y, ang.z);
        self.motor_left.set_power(l)?;
        self.motor_right.set_power(r)?;
        Ok(())
    }
}
