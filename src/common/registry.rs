#![allow(dead_code)]

lazy_static::lazy_static! {
    pub(crate) static ref COMPONENT_REGISTRY: ComponentRegistry = {
        let mut r = ComponentRegistry::new();
        crate::common::board::register_models(&mut r);
        crate::common::motor::register_models(&mut r);
        r
    };
}

#[derive(Debug, Eq, PartialEq)]
pub enum RegistryError {
    ModelNotFound,
    ModelAlreadyRegistered(&'static str),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::ModelNotFound => {
                write!(f, "RegistryError : Model not found")
            }
            RegistryError::ModelAlreadyRegistered(model) => {
                write!(f, "RegistryError : model {} already exists", model)
            }
        }
    }
}
impl Error for RegistryError {}

use core::fmt;
use std::{collections::BTreeMap as Map, error::Error};

use super::{board::BoardType, config::StaticComponentConfig, motor::MotorType};

type MotorConstructor =
    dyn Fn(&StaticComponentConfig, Option<BoardType>) -> anyhow::Result<MotorType>;

type BoardConstructor = dyn Fn(&StaticComponentConfig) -> anyhow::Result<BoardType>;

pub(crate) struct ComponentRegistry {
    motors: Map<&'static str, &'static MotorConstructor>,
    board: Map<&'static str, &'static BoardConstructor>,
}

unsafe impl Sync for ComponentRegistry {}

impl ComponentRegistry {
    pub(crate) fn new() -> Self {
        Self {
            motors: Map::new(),
            board: Map::new(),
        }
    }
    pub(crate) fn register_motor(
        &mut self,
        model: &'static str,
        constructor: &'static MotorConstructor,
    ) -> Result<(), RegistryError> {
        if self.motors.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.motors.insert(model, constructor);
        Ok(())
    }

    pub(crate) fn register_board(
        &mut self,
        model: &'static str,
        constructor: &'static BoardConstructor,
    ) -> Result<(), RegistryError> {
        if self.board.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.board.insert(model, constructor);
        Ok(())
    }
    pub(crate) fn get_motor_constructor(
        &self,
        model: &'static str,
    ) -> Result<&'static MotorConstructor, RegistryError> {
        if let Some(ctor) = self.motors.get(model) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound)
    }

    pub(crate) fn get_board_constructor(
        &self,
        model: &'static str,
    ) -> Result<&'static BoardConstructor, RegistryError> {
        if let Some(ctor) = self.board.get(model) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound)
    }
}

#[cfg(test)]
mod tests {
    use crate::common;
    use crate::common::config::StaticComponentConfig;
    use crate::common::registry::{ComponentRegistry, RegistryError, COMPONENT_REGISTRY};
    #[test_log::test]
    fn test_registry() -> anyhow::Result<()> {
        let mut registry = ComponentRegistry::new();

        let ctor = registry.get_motor_constructor("fake");
        assert!(ctor.is_err());
        assert_eq!(ctor.err().unwrap(), RegistryError::ModelNotFound);
        common::motor::register_models(&mut registry);

        let ctor = registry.get_motor_constructor("fake");
        assert!(ctor.is_ok());

        let ret = registry.register_motor("fake", &|_, _| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            RegistryError::ModelAlreadyRegistered("fake")
        );

        let ret = registry.register_motor("fake2", &|_, _| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_ok());

        let ctor = registry.get_board_constructor("fake");
        assert!(ctor.is_err());
        assert_eq!(ctor.err().unwrap(), RegistryError::ModelNotFound);
        common::board::register_models(&mut registry);

        let ctor = registry.get_board_constructor("fake");
        assert!(ctor.is_ok());

        let ret = registry.register_board("fake", &|_| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            RegistryError::ModelAlreadyRegistered("fake")
        );

        let ret = registry.register_board("fake2", &|_| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_ok());

        let cfg = StaticComponentConfig::default();

        let ctor = registry.get_motor_constructor("fake2");
        assert!(ctor.is_ok());

        let ret = ctor.unwrap()(&cfg, None);

        assert!(ret.is_err());
        assert_eq!(format!("{}", ret.err().unwrap()), "not implemented");

        let ctor = registry.get_board_constructor("fake2");
        assert!(ctor.is_ok());

        let ret = ctor.unwrap()(&cfg);

        assert!(ret.is_err());
        assert_eq!(format!("{}", ret.err().unwrap()), "not implemented");

        Ok(())
    }

    #[test_log::test]
    fn test_lazy_init() {
        let ctor = COMPONENT_REGISTRY.get_motor_constructor("fake");
        assert!(ctor.is_ok());

        let ctor = COMPONENT_REGISTRY.get_board_constructor("fake");
        assert!(ctor.is_ok());
    }
}
