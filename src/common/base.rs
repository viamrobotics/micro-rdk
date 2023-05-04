#![allow(dead_code)]
use crate::common::status::Status;
use crate::proto::common::v1::Vector3;
use log::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub trait Base: Status {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
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
    fn stop(&mut self) -> anyhow::Result<()> {
        self.get_mut().unwrap().stop()
    }
}

impl Base for FakeBase {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        info!(
            "Setting power following lin vec {:?} and ang {:?}",
            lin, ang
        );
        Ok(())
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping base");
        Ok(())
    }
}

impl Status for FakeBase {
    fn get_status(&mut self) -> anyhow::Result<Option<prost_types::Struct>> {
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
