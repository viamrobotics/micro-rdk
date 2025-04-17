use super::generic::DoCommand;

#[cfg(feature = "builtin-components")]
use crate::common::{
    config::ConfigType,
    registry::{ComponentRegistry, Dependency},
};
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub static COMPONENT_NAME: &str = "switch";

pub type SwitchType = Arc<Mutex<dyn Switch>>;

#[derive(Debug, Error)]
pub enum SwitchError {
    #[error("index `{0}` is out of bounds; range is 0-{1}")]
    InvalidPosition(u32, u32),
    #[error("{0}")]
    Other(String),
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

pub trait Switch: DoCommand + Send {
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
            return Err(SwitchError::Other("`fail_new` attribute is set".to_string()));
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
            return Err(SwitchError::InvalidPosition(pos, self.num_pos));
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
