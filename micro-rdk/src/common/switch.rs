use super::{generic::DoCommand, status::Status};

use std::sync::{Arc, Mutex};
use thiserror::Error;
#[cfg(feature = "builtin-components")]
use {
    crate::{
        common::{
            config::ConfigType,
            registry::{ComponentRegistry, Dependency},
            status::StatusError,
        },
        google,
    },
    std::collections::HashMap,
};

pub static COMPONENT_NAME: &str = "switch";

pub type SwitchType = Arc<Mutex<dyn Switch>>;

#[derive(Debug, Error)]
pub enum SwitchError {
    #[error("failed to flip switch")]
    FailedFlip,
    #[error("`{0}` is not a valid position")]
    InvalidPosition(u32),
    #[error("test error")]
    TestError,
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[cfg(feature = "builtin-components")]
pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_switch("fake", &FakeSwitch::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
}

pub trait Switch: Status + DoCommand + Send {
    fn set_position(&mut self, pos: u32) -> Result<(), SwitchError>;
    fn get_position(&self) -> Result<u32, SwitchError>;
    fn get_num_positions(&self) -> Result<u32, SwitchError>;
}

impl<L> Switch for Mutex<L>
where
    L: ?Sized + Switch,
{
    fn set_position(&mut self, pos: u32) -> Result<(), SwitchError> {
        self.get_mut().unwrap().set_position(pos)
    }
    fn get_position(&self) -> Result<u32, SwitchError> {
        self.lock().unwrap().get_position()
    }
    fn get_num_positions(&self) -> Result<u32, SwitchError> {
        self.lock().unwrap().get_num_positions()
    }
}

impl<A> Switch for Arc<Mutex<A>>
where
    A: ?Sized + Switch,
{
    fn set_position(&mut self, pos: u32) -> Result<(), SwitchError> {
        self.lock().unwrap().set_position(pos)
    }
    fn get_position(&self) -> Result<u32, SwitchError> {
        self.lock().unwrap().get_position()
    }
    fn get_num_positions(&self) -> Result<u32, SwitchError> {
        self.lock().unwrap().get_num_positions()
    }
}

#[cfg(feature = "builtin-components")]
#[derive(DoCommand)]
pub struct FakeSwitch {
    num_pos: u32,
    curr_pos: u32,
}

#[cfg(feature = "builtin-components")]
impl FakeSwitch {
    pub(crate) fn from_config(
        cfg: ConfigType,
        _deps: Vec<Dependency>,
    ) -> Result<SwitchType, SwitchError> {
        if cfg.get_attribute::<bool>("fail_new").unwrap_or(false) {
            return Err(SwitchError::TestError);
        }
        let num_pos = cfg.get_attribute::<u32>("position_count").unwrap_or(2);
        Ok(Arc::new(Mutex::new(Self {
            num_pos,
            curr_pos: 0,
        })))
    }
}

#[cfg(feature = "builtin-components")]
impl Switch for FakeSwitch {
    fn set_position(&mut self, pos: u32) -> Result<(), SwitchError> {
        if pos >= self.num_pos {
            return Err(SwitchError::InvalidPosition(pos));
        }
        self.curr_pos = pos;
        Ok(())
    }
    fn get_position(&self) -> Result<u32, SwitchError> {
        Ok(self.curr_pos)
    }
    fn get_num_positions(&self) -> Result<u32, SwitchError> {
        Ok(self.num_pos)
    }
}

#[cfg(feature = "builtin-components")]
impl Status for FakeSwitch {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        let mut hm = HashMap::new();
        hm.insert(
            "curr_pos".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(
                    self.curr_pos.into(),
                )),
            },
        );
        hm.insert(
            "num_pos".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(
                    self.num_pos.into(),
                )),
            },
        );

        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}
