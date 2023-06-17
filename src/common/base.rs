#![allow(dead_code)]
use crate::common::status::Status;
use crate::common::stop::Stoppable;
use crate::proto::common::v1::Vector3;
use log::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub static COMPONENT_NAME: &str = "base";

pub trait Base: Status + Stoppable {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()>;
}

pub(crate) type BaseType = Arc<Mutex<dyn Base>>;

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

impl Base for FakeBase {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        debug!(
            "Setting power following lin vec {:?} and ang {:?}",
            lin, ang
        );
        Ok(())
    }
}

impl Stoppable for FakeBase {
    fn stop(&mut self) -> anyhow::Result<()> {
        debug!("Stopping base");
        Ok(())
    }
}

impl Status for FakeBase {
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
