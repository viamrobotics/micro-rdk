#![allow(dead_code)]
use crate::common::actuator::Actuator;
use crate::common::status::Status;
use crate::google;
use crate::proto::common::v1::Vector3;
use anyhow::Ok;
use log::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::generic::DoCommand;

pub static COMPONENT_NAME: &str = "base";

pub trait Base: Status + Actuator + DoCommand {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()>;
}

pub type BaseType = Arc<Mutex<dyn Base>>;

// TODO(RSDK-5648) - Store power from set_power call on struct and register as "fake" model
#[derive(DoCommand)]
pub struct FakeBase {}

impl FakeBase {
    pub fn new() -> Self {
        FakeBase {}
    }
}
impl Default for FakeBase {
    fn default() -> Self {
        Self::new()
    }
}

impl<L> Base for Mutex<L>
where
    L: Base,
{
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        self.get_mut().unwrap().set_power(lin, ang)
    }
}

impl<L> Base for Arc<Mutex<L>>
where
    L: Base,
{
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        self.lock().unwrap().set_power(lin, ang)
    }
}

impl Base for FakeBase {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        debug!(
            "Setting power following lin vec {:?} and ang {:?}",
            lin, ang
        );
        Ok(())
    }
}

impl Actuator for FakeBase {
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        debug!("Stopping base");
        Ok(())
    }
}

impl Status for FakeBase {
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
