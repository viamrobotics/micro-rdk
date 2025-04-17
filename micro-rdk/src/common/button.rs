use super::generic::DoCommand;

#[cfg(feature = "builtin-components")]
use crate::common::{
    config::ConfigType,
    registry::{ComponentRegistry, Dependency},
};
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub static COMPONENT_NAME: &str = "button";

pub type ButtonType = Arc<Mutex<dyn Button>>;

#[derive(Debug, Error)]
pub enum ButtonError {
    #[error("{0}")]
    Other(String),
}

#[cfg(feature = "builtin-components")]
pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_button("fake", &FakeButton::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
}

pub trait Button: DoCommand + Send {
    fn push(&mut self) -> Result<(), ButtonError>;
}

impl<L> Button for Mutex<L>
where
    L: ?Sized + Button,
{
    fn push(&mut self) -> Result<(), ButtonError> {
        self.get_mut().unwrap().push()
    }
}

impl<A> Button for Arc<Mutex<A>>
where
    A: ?Sized + Button,
{
    fn push(&mut self) -> Result<(), ButtonError> {
        self.lock().unwrap().push()
    }
}

#[cfg(feature = "builtin-components")]
#[derive(DoCommand)]
pub struct FakeButton {
    count: u32,
}

#[cfg(feature = "builtin-components")]
impl FakeButton {
    fn new() -> Self {
        Self { count: 0 }
    }
    pub(crate) fn from_config(
        cfg: ConfigType,
        _deps: Vec<Dependency>,
    ) -> Result<ButtonType, ButtonError> {
        if cfg.get_attribute::<bool>("fail_new").unwrap_or(false) {
            return Err(ButtonError::Other(
                "`fail_new` attribute is set".to_string(),
            ));
        }
        Ok(Arc::new(Mutex::new(Self::new())))
    }
}

#[cfg(feature = "builtin-components")]
impl Button for FakeButton {
    fn push(&mut self) -> Result<(), ButtonError> {
        self.count += 1;
        log::info!("push count: {}", self.count);
        Ok(())
    }
}
