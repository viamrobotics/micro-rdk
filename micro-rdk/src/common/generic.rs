use std::sync::{Arc, Mutex};

use crate::google::protobuf::Struct;

#[cfg(feature = "builtin-components")]
use {
    super::{
        config::ConfigType,
        registry::{ComponentRegistry, Dependency},
    },
    crate::google::protobuf::{value::Kind, Value},
    std::collections::HashMap,
};

use thiserror::Error;

pub static COMPONENT_NAME: &str = "generic";

#[derive(Debug, Error)]
pub enum GenericError {
    #[error("Generic: method {0} unimplemented")]
    MethodUnimplemented(&'static str),
    #[error("Generic other error: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}
#[cfg(feature = "builtin-components")]
pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_generic_component("fake", &FakeGenericComponent::from_config)
        .is_err()
    {
        log::error!("model fake is already registered")
    }
}

pub trait DoCommand {
    /// do_command custom commands outside of a strict API. Takes a command struct that can be interpreted
    /// as a map of method name keys to argument values.
    fn do_command(
        &mut self,
        _command_struct: Option<Struct>,
    ) -> Result<Option<Struct>, GenericError> {
        Err(GenericError::MethodUnimplemented("do_command"))
    }
}

impl<L> DoCommand for Mutex<L>
where
    L: ?Sized + DoCommand,
{
    fn do_command(
        &mut self,
        command_struct: Option<Struct>,
    ) -> Result<Option<Struct>, GenericError> {
        self.get_mut().unwrap().do_command(command_struct)
    }
}

impl<A> DoCommand for Arc<Mutex<A>>
where
    A: ?Sized + DoCommand,
{
    fn do_command(
        &mut self,
        command_struct: Option<Struct>,
    ) -> Result<Option<Struct>, GenericError> {
        self.lock().unwrap().do_command(command_struct)
    }
}

pub trait GenericComponent: DoCommand {}

pub type GenericComponentType = Arc<Mutex<dyn GenericComponent>>;

impl<L> GenericComponent for Mutex<L> where L: ?Sized + GenericComponent {}

impl<A> GenericComponent for Arc<Mutex<A>> where A: ?Sized + GenericComponent {}

#[cfg(feature = "builtin-components")]
pub struct FakeGenericComponent {}

#[cfg(feature = "builtin-components")]
impl FakeGenericComponent {
    pub(crate) fn from_config(
        _: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<GenericComponentType, GenericError> {
        Ok(Arc::new(Mutex::new(FakeGenericComponent {})))
    }
}

#[cfg(feature = "builtin-components")]
impl GenericComponent for FakeGenericComponent {}

#[cfg(feature = "builtin-components")]
impl DoCommand for FakeGenericComponent {
    fn do_command(
        &mut self,
        command_struct: Option<Struct>,
    ) -> Result<Option<Struct>, GenericError> {
        let mut res = HashMap::new();
        if let Some(command_struct) = command_struct.as_ref() {
            for (key, val) in &command_struct.fields {
                match key.as_str() {
                    "ping" => {
                        res.insert(
                            "ping".to_string(),
                            Value {
                                kind: Some(Kind::StringValue("pinged".to_string())),
                            },
                        );
                    }
                    "echo" => {
                        res.insert("echoed".to_string(), val.to_owned());
                    }
                    _ => {}
                };
            }
        }
        Ok(Some(Struct { fields: res }))
    }
}
