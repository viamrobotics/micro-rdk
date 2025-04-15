use super::{
    config::ConfigType,
    generic::DoCommand,
    registry::{ComponentRegistry, Dependency},
    status::{Status, StatusError},
};
use crate::google;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use thiserror::Error;

pub static COMPONENT_NAME: &str = "button";

pub type ButtonType = Arc<Mutex<dyn Button>>;

#[derive(Debug, Error)]
pub enum ButtonError {
    #[error("failed to press button")]
    FailedPress,
    #[error("test error")]
    TestError,
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
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

pub trait Button: Status + DoCommand + Send {
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
            return Err(ButtonError::TestError);
        }
        Ok(Arc::new(Mutex::new(Self::new())))
    }
}

#[cfg(feature = "builtin-components")]
impl Button for FakeButton {
    fn push(&mut self) -> Result<(), ButtonError> {
        self.count += 1;
        Ok(())
    }
}

#[cfg(feature = "builtin-components")]
impl Status for FakeButton {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        let mut hm = HashMap::new();

        hm.insert(
            "count".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(
                    self.count.into(),
                )),
            },
        );

        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}
