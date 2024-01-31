use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::google::protobuf::{value::Kind, Struct, Value};

use super::{
    config::ConfigType,
    registry::{ComponentRegistry, Dependency},
    status::Status,
};

pub static COMPONENT_NAME: &str = "generic";

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
    fn do_command(&mut self, _command_struct: Option<Struct>) -> anyhow::Result<Option<Struct>> {
        anyhow::bail!("do_command unimplemented")
    }
}

impl<L> DoCommand for Mutex<L>
where
    L: ?Sized + DoCommand,
{
    fn do_command(&mut self, command_struct: Option<Struct>) -> anyhow::Result<Option<Struct>> {
        self.get_mut().unwrap().do_command(command_struct)
    }
}

impl<A> DoCommand for Arc<Mutex<A>>
where
    A: ?Sized + DoCommand,
{
    fn do_command(&mut self, command_struct: Option<Struct>) -> anyhow::Result<Option<Struct>> {
        self.lock().unwrap().do_command(command_struct)
    }
}

pub trait GenericComponent: DoCommand + Status {}

pub type GenericComponentType = Arc<Mutex<dyn GenericComponent>>;

impl<L> GenericComponent for Mutex<L> where L: ?Sized + GenericComponent {}

impl<A> GenericComponent for Arc<Mutex<A>> where A: ?Sized + GenericComponent {}

pub struct FakeGenericComponent {}

impl FakeGenericComponent {
    pub(crate) fn from_config(
        _: ConfigType,
        _: Vec<Dependency>,
    ) -> anyhow::Result<GenericComponentType> {
        Ok(Arc::new(Mutex::new(FakeGenericComponent {})))
    }
}

impl GenericComponent for FakeGenericComponent {}

impl DoCommand for FakeGenericComponent {
    fn do_command(&mut self, command_struct: Option<Struct>) -> anyhow::Result<Option<Struct>> {
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

impl Status for FakeGenericComponent {
    fn get_status(&self) -> anyhow::Result<Option<Struct>> {
        Ok(Some(Struct {
            fields: HashMap::new(),
        }))
    }
}
