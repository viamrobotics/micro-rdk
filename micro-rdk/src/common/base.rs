#![allow(dead_code)]
#[cfg(feature = "builtin-components")]
use {super::actuator::ActuatorError, crate::google, log::*, std::collections::HashMap};

use super::generic::DoCommand;
use crate::common::actuator::Actuator;
use crate::common::status::Status;
use crate::proto::common::v1::Vector3;
use std::sync::{Arc, Mutex};

pub static COMPONENT_NAME: &str = "base";

pub trait Base: Status + Actuator + DoCommand {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> Result<(), BaseError>;
}

pub type BaseType = Arc<Mutex<dyn Base>>;

#[derive(Error, Debug)]
pub enum BaseError {
    #[error(transparent)]
    BaseMotorError(#[from] MotorError),
    #[error(transparent)]
    BaseConfigAttributeError(#[from] AttributeError),
    #[error("config error: {0}")]
    BaseConfigError(&'static str),
}

// TODO(RSDK-5648) - Store power from set_power call on struct and register as "fake" model
#[cfg(feature = "builtin-components")]
#[derive(DoCommand)]
pub struct FakeBase {}

#[cfg(feature = "builtin-components")]
impl FakeBase {
    pub fn new() -> Self {
        FakeBase {}
    }
}
#[cfg(feature = "builtin-components")]
impl Default for FakeBase {
    fn default() -> Self {
        Self::new()
    }
}

impl<L> Base for Mutex<L>
where
    L: Base,
{
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> Result<(), BaseError> {
        self.get_mut().unwrap().set_power(lin, ang)
    }
}

impl<L> Base for Arc<Mutex<L>>
where
    L: Base,
{
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> Result<(), BaseError> {
        self.lock().unwrap().set_power(lin, ang)
    }
}

#[cfg(feature = "builtin-components")]
impl Base for FakeBase {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> Result<(), BaseError> {
        debug!(
            "Setting power following lin vec {:?} and ang {:?}",
            lin, ang
        );
        Ok(())
    }
}

#[cfg(feature = "builtin-components")]
impl Actuator for FakeBase {
    fn is_moving(&mut self) -> Result<bool, ActuatorError> {
        Ok(false)
    }
    fn stop(&mut self) -> Result<(), ActuatorError> {
        debug!("Stopping base");
        Ok(())
    }
}

#[cfg(feature = "builtin-components")]
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
