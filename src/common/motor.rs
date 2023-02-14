#![allow(dead_code)]
use crate::common::status::Status;
use log::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::board::BoardType;
use super::config::{Component, ConfigType};
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

pub struct FakeMotor {
    pos: f64,
    power: f64,
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
