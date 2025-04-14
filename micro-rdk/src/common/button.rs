use super::{
    config::ConfigType,
    generic::DoCommand,
    status::{Status, StatusError},
};
use crate::google;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use thiserror::Error;

pub type ButtonType = Arc<Mutex<dyn Button>>;

#[derive(Debug, Error)]
pub enum ButtonError {
    #[error("failed to press button")]
    FailedPress,
    #[error("test error")]
    TestError,
}

pub trait Button: Status + DoCommand {
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

#[derive(DoCommand)]
pub struct FakeButton {
    count: u32,
}

impl FakeButton {
    fn new() -> Self {
        Self { count: 0 }
    }
    pub(crate) fn from_config(cfg: ConfigType) -> Result<ButtonType, ButtonError> {
        if cfg.get_attribute::<bool>("fail_new").unwrap_or(false) {
            return Err(ButtonError::TestError);
        }
        Ok(Arc::new(Mutex::new(Self::new())))
    }
}

impl Button for FakeButton {
    fn push(&mut self) -> Result<(), ButtonError> {
        self.count += 1;
        Ok(())
    }
}

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
