#![allow(dead_code)]
use thiserror::Error;

lazy_static::lazy_static! {
    pub(crate) static ref COMPONENT_REGISTRY: ComponentRegistry = {
        let mut r = ComponentRegistry::new();
        crate::common::board::register_models(&mut r);
        crate::common::encoder::register_models(&mut r);
        crate::common::motor::register_models(&mut r);
        crate::common::sensor::register_models(&mut r);
        crate::common::movement_sensor::register_models(&mut r);
        crate::common::mpu6050::register_models(&mut r);
        crate::common::adxl345::register_models(&mut r);
        #[cfg(esp32)]
        crate::esp32::board::register_models(&mut r);
        #[cfg(esp32)]
        crate::esp32::motor::register_models(&mut r);
        #[cfg(esp32)]
        crate::esp32::encoder::register_models(&mut r);
        #[cfg(esp32)]
        crate::esp32::single_encoder::register_models(&mut r);
        #[cfg(esp32)]
        crate::esp32::base::register_models(&mut r);
        r
    };
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum RegistryError {
    #[error("RegistryError : Model '{0}' not found")]
    ModelNotFound(String),
    #[error("RegistryError : model '{0}' already exists")]
    ModelAlreadyRegistered(&'static str),
    #[error("RegistryError: model '{0}' dependency getter already registered")]
    ModelDependencyFuncRegistered(&'static str),
    #[error("RegistryError: dependencies unsupported for component type '{0}'")]
    ComponentTypeNotInDependencies(&'static str),
    #[error("RegistryError: model '{0}' not found in dependencies under component type '{1}'")]
    ModelNotFoundInDependencies(String, &'static str),
}

use std::collections::BTreeMap as Map;

use crate::proto::common::v1::ResourceName;

use super::{
    base::BaseType, board::BoardType, config::ConfigType, encoder::EncoderType, motor::MotorType,
    movement_sensor::MovementSensorType, robot::Resource, sensor::SensorType,
};

pub fn get_board_from_dependencies(deps: Vec<Dependency>) -> Option<BoardType> {
    for Dependency(_, dep) in deps {
        match dep {
            Resource::Board(b) => return Some(b.clone()),
            _ => continue,
        }
    }
    None
}

// ResourceKey is an identifier for a component to be registered to a robot. The
// first element is a string representing the component type (arm, motor, etc.)
// and the second element is its name.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ResourceKey(pub &'static str, pub String);

impl ResourceKey {
    pub fn new(model: &str, name: String) -> Result<Self, anyhow::Error> {
        let model_str = match model {
            "motor" => crate::common::motor::COMPONENT_NAME,
            "board" => crate::common::board::COMPONENT_NAME,
            "encoder" => crate::common::encoder::COMPONENT_NAME,
            "movement_sensor" => crate::common::movement_sensor::COMPONENT_NAME,
            "sensor" => crate::common::sensor::COMPONENT_NAME,
            "base" => crate::common::base::COMPONENT_NAME,
            &_ => {
                anyhow::bail!("component type {} is not supported yet", model.to_string());
            }
        };
        Ok(Self(model_str, name))
    }
}

impl TryFrom<ResourceName> for ResourceKey {
    type Error = anyhow::Error;
    fn try_from(value: ResourceName) -> Result<Self, Self::Error> {
        let comp_type: &str = &value.subtype;
        let comp_name = match comp_type {
            "motor" => crate::common::motor::COMPONENT_NAME,
            "sensor" => crate::common::sensor::COMPONENT_NAME,
            "movement_sensor" => crate::common::movement_sensor::COMPONENT_NAME,
            "encoder" => crate::common::encoder::COMPONENT_NAME,
            "base" => crate::common::base::COMPONENT_NAME,
            _ => {
                anyhow::bail!("component type {} is not supported yet", comp_type);
            }
        };
        Ok(Self(comp_name, value.name))
    }
}

pub struct Dependency(pub ResourceKey, pub Resource);

type BoardConstructor = dyn Fn(ConfigType) -> anyhow::Result<BoardType>;

type MotorConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<MotorType>;

type SensorConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<SensorType>;

type MovementSensorConstructor =
    dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<MovementSensorType>;

type EncoderConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<EncoderType>;

type BaseConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<BaseType>;

type DependenciesFromConfig = dyn Fn(ConfigType) -> Vec<ResourceKey>;

pub(crate) struct ComponentRegistry {
    motors: Map<&'static str, &'static MotorConstructor>,
    board: Map<&'static str, &'static BoardConstructor>,
    sensor: Map<&'static str, &'static SensorConstructor>,
    movement_sensors: Map<&'static str, &'static MovementSensorConstructor>,
    encoders: Map<&'static str, &'static EncoderConstructor>,
    bases: Map<&'static str, &'static BaseConstructor>,
    dependencies: Map<&'static str, Map<&'static str, &'static DependenciesFromConfig>>,
}

unsafe impl Sync for ComponentRegistry {}

impl ComponentRegistry {
    pub(crate) fn new() -> Self {
        let mut dependency_func_map = Map::new();
        dependency_func_map.insert(crate::common::motor::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::movement_sensor::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::encoder::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::sensor::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::base::COMPONENT_NAME, Map::new());
        Self {
            motors: Map::new(),
            board: Map::new(),
            sensor: Map::new(),
            movement_sensors: Map::new(),
            encoders: Map::new(),
            bases: Map::new(),
            dependencies: dependency_func_map,
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

    pub(crate) fn register_sensor(
        &mut self,
        model: &'static str,
        constructor: &'static SensorConstructor,
    ) -> Result<(), RegistryError> {
        if self.sensor.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.sensor.insert(model, constructor);
        Ok(())
    }

    pub(crate) fn register_movement_sensor(
        &mut self,
        model: &'static str,
        constructor: &'static MovementSensorConstructor,
    ) -> Result<(), RegistryError> {
        if self.movement_sensors.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.movement_sensors.insert(model, constructor);
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

    pub(crate) fn register_encoder(
        &mut self,
        model: &'static str,
        constructor: &'static EncoderConstructor,
    ) -> Result<(), RegistryError> {
        if self.encoders.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.encoders.insert(model, constructor);
        Ok(())
    }

    pub(crate) fn register_base(
        &mut self,
        model: &'static str,
        constructor: &'static BaseConstructor,
    ) -> Result<(), RegistryError> {
        if self.bases.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.bases.insert(model, constructor);
        Ok(())
    }

    pub(crate) fn register_dependency_getter(
        &mut self,
        component_type: &'static str,
        model: &'static str,
        getter: &'static DependenciesFromConfig,
    ) -> Result<(), RegistryError> {
        if !self.dependencies.contains_key(component_type) {
            return Err(RegistryError::ComponentTypeNotInDependencies(
                component_type,
            ));
        }
        let comp_deps = self.dependencies.get_mut(component_type).unwrap();
        if comp_deps.contains_key(model) {
            return Err(RegistryError::ModelDependencyFuncRegistered(model));
        }
        let _ = comp_deps.insert(model, getter);
        Ok(())
    }

    pub(crate) fn get_dependency_function(
        &self,
        component_type: &'static str,
        model: String,
    ) -> Result<&'static DependenciesFromConfig, RegistryError> {
        let model_name: &str = &model;
        if !self.dependencies.contains_key(component_type) {
            return Err(RegistryError::ComponentTypeNotInDependencies(
                component_type,
            ));
        }
        let comp_deps = self.dependencies.get(component_type).unwrap();
        if let Some(func) = comp_deps.get(model_name) {
            return Ok(*func);
        }
        Err(RegistryError::ModelNotFoundInDependencies(
            model,
            component_type,
        ))
    }

    pub(crate) fn get_board_constructor(
        &self,
        model: String,
    ) -> Result<&'static BoardConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.board.get(model_name) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model))
    }

    pub(crate) fn get_motor_constructor(
        &self,
        model: String,
    ) -> Result<&'static MotorConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.motors.get(model_name) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model))
    }

    pub(crate) fn get_sensor_constructor(
        &self,
        model: String,
    ) -> Result<&'static SensorConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.sensor.get(model_name) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model))
    }

    pub(crate) fn get_movement_sensor_constructor(
        &self,
        model: String,
    ) -> Result<&'static MovementSensorConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.movement_sensors.get(model_name) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model))
    }

    pub(crate) fn get_encoder_constructor(
        &self,
        model: String,
    ) -> Result<&'static EncoderConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.encoders.get(model_name) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model))
    }

    pub(crate) fn get_base_constructor(
        &self,
        model: String,
    ) -> Result<&'static BaseConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.bases.get(model_name) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model))
    }
}

#[cfg(test)]
mod tests {
    use crate::common;
    use crate::common::config::{ConfigType, StaticComponentConfig};
    use crate::common::registry::{ComponentRegistry, RegistryError, COMPONENT_REGISTRY};

    lazy_static::lazy_static! {
        static ref EMPTY_CONFIG: StaticComponentConfig = StaticComponentConfig::default();
    }

    #[test_log::test]
    fn test_registry() -> anyhow::Result<()> {
        let mut registry = ComponentRegistry::new();

        let ctor = registry.get_motor_constructor("fake".to_string());
        assert!(ctor.is_err());
        assert_eq!(
            ctor.err().unwrap(),
            RegistryError::ModelNotFound("fake".to_string())
        );
        common::motor::register_models(&mut registry);

        let ctor = registry.get_motor_constructor("fake".to_string());
        assert!(ctor.is_ok());

        let ret = registry.register_motor("fake", &|_, _| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            RegistryError::ModelAlreadyRegistered("fake")
        );

        let ret = registry.register_motor("fake2", &|_, _| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_ok());

        let ctor = registry.get_board_constructor("fake".to_string());
        assert!(ctor.is_err());
        assert_eq!(
            ctor.err().unwrap(),
            RegistryError::ModelNotFound("fake".to_string())
        );
        common::board::register_models(&mut registry);

        let ctor = registry.get_board_constructor("fake".to_string());
        assert!(ctor.is_ok());

        let ret = registry.register_board("fake", &|_| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            RegistryError::ModelAlreadyRegistered("fake")
        );

        let ret = registry.register_board("fake2", &|_| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_ok());

        let ctor = registry.get_motor_constructor("fake2".to_string());
        assert!(ctor.is_ok());

        let ret = ctor.unwrap()(ConfigType::Static(&EMPTY_CONFIG), Vec::new());

        assert!(ret.is_err());
        assert_eq!(format!("{}", ret.err().unwrap()), "not implemented");

        let ctor = registry.get_board_constructor("fake2".to_string());
        assert!(ctor.is_ok());

        let ret = ctor.unwrap()(ConfigType::Static(&EMPTY_CONFIG));

        assert!(ret.is_err());
        assert_eq!(format!("{}", ret.err().unwrap()), "not implemented");

        Ok(())
    }

    #[test_log::test]
    fn test_lazy_init() {
        let ctor = COMPONENT_REGISTRY.get_motor_constructor("fake".to_string());
        assert!(ctor.is_ok());

        let ctor = COMPONENT_REGISTRY.get_board_constructor("fake".to_string());
        assert!(ctor.is_ok());
    }
}
