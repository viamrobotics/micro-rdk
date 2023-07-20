#![allow(dead_code)]
use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum RegistryError {
    #[error("RegistryError : Model '{0}' not found")]
    ModelNotFound(String),
    #[error("RegistryError : model '{0}' already exists")]
    ModelAlreadyRegistered(String),
    #[error("RegistryError: model '{0}' dependency getter already registered")]
    ModelDependencyFuncRegistered(String),
    #[error("RegistryError: dependencies unsupported for component type '{0}'")]
    ComponentTypeNotInDependencies(String),
    #[error("RegistryError: model '{0}' not found in dependencies under component type '{1}'")]
    ModelNotFoundInDependencies(String, String),
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

/// Fn that returns a BoardType, Arc<Mutex<dyn Board>>
type BoardConstructor = dyn Fn(ConfigType) -> anyhow::Result<BoardType>;

/// Fn that returns a MotorType, Arc<Mutex<dyn Motor>>
type MotorConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<MotorType>;

/// Fn that returns a SensorType, Arc<Mutex<dyn Sensor>
type SensorConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<SensorType>;

/// Fn that returns a MovementSensorType, Arc<Mutex<dyn MovementSensor>
type MovementSensorConstructor =
    dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<MovementSensorType>;

/// Fn that returns an EncoderType, Arc<Mutex<dyn Encoder>
type EncoderConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<EncoderType>;

/// Fn that returns an BaseType, Arc<Mutex<dyn Base>
type BaseConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> anyhow::Result<BaseType>;

/// Fn that returns an Vec of ResourceKeys, ('static str, String)
type DependenciesFromConfig = dyn Fn(ConfigType) -> Vec<ResourceKey>;

/// Holds mappings for all of a Robot's Components and their dependency mappings
pub struct ComponentRegistry<'model, 'ctor, 'dep> {
    motors: Map<&'model str, &'ctor MotorConstructor>,
    board: Map<&'model str, &'ctor BoardConstructor>,
    sensor: Map<&'model str, &'ctor SensorConstructor>,
    movement_sensors: Map<&'model str, &'ctor MovementSensorConstructor>,
    encoders: Map<&'model str, &'ctor EncoderConstructor>,
    bases: Map<&'model str, &'ctor BaseConstructor>,
    dependencies: Map<&'dep str, Map<&'dep str, &'dep DependenciesFromConfig>>,
}

unsafe impl<'model, 'ctor, 'dep> Sync for ComponentRegistry<'model, 'ctor, 'dep> {}

impl<'model: 'dep, 'ctor, 'dep> ComponentRegistry<'model, 'ctor, 'dep> {
    pub fn new() -> Self {
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
    pub fn default() -> Self {
        let mut r = Self::new();
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
    }
    pub fn register_motor(
        &mut self,
        model: &'model str,
        constructor: &'ctor MotorConstructor,
    ) -> Result<(), RegistryError> {
        if self.motors.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model.to_string()));
        }
        let _ = self.motors.insert(&model, constructor);
        Ok(())
    }

    pub fn register_sensor(
        &mut self,
        model: &'model str,
        constructor: &'ctor SensorConstructor,
    ) -> Result<(), RegistryError> {
        if self.sensor.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model.to_string()));
        }
        let _ = self.sensor.insert(model, constructor);
        Ok(())
    }

    pub fn register_movement_sensor(
        &mut self,
        model: &'model str,
        constructor: &'ctor MovementSensorConstructor,
    ) -> Result<(), RegistryError> {
        if self.movement_sensors.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model.to_string()));
        }
        let _ = self.movement_sensors.insert(model, constructor);
        Ok(())
    }

    pub fn register_board(
        &mut self,
        model: &'model str,
        constructor: &'ctor BoardConstructor,
    ) -> Result<(), RegistryError> {
        if self.board.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model.to_string()));
        }
        let _ = self.board.insert(model, constructor);
        Ok(())
    }

    pub fn register_encoder(
        &mut self,
        model: &'model str,
        constructor: &'ctor EncoderConstructor,
    ) -> Result<(), RegistryError> {
        if self.encoders.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model.to_string()));
        }
        let _ = self.encoders.insert(model, constructor);
        Ok(())
    }

    pub fn register_base(
        &mut self,
        model: &'model str,
        constructor: &'ctor BaseConstructor,
    ) -> Result<(), RegistryError> {
        if self.bases.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model.to_string()));
        }
        let _ = self.bases.insert(model, constructor);
        Ok(())
    }

    pub fn register_dependency_getter(
        &mut self,
        component_type: &'model str,
        model: &'model str,
        getter: &'dep DependenciesFromConfig,
    ) -> Result<(), RegistryError> {
        if !self.dependencies.contains_key(component_type) {
            return Err(RegistryError::ComponentTypeNotInDependencies(
                component_type.to_string(),
            ));
        }
        let comp_deps = self.dependencies.get_mut(component_type).unwrap();
        if comp_deps.contains_key(model) {
            return Err(RegistryError::ModelDependencyFuncRegistered(
                model.to_string(),
            ));
        }
        let _ = comp_deps.insert(model, getter);
        Ok(())
    }

    pub(crate) fn get_dependency_function(
        &self,
        component_type: &str,
        model: &'model str,
    ) -> Result<&DependenciesFromConfig, RegistryError> {
        if !self.dependencies.contains_key(component_type) {
            return Err(RegistryError::ComponentTypeNotInDependencies(
                component_type.to_string(),
            ));
        }
        let comp_deps = self.dependencies.get(component_type).unwrap();
        if let Some(func) = comp_deps.get(model) {
            return Ok(*func);
        }
        Err(RegistryError::ModelNotFoundInDependencies(
            model.to_string(),
            component_type.to_string(),
        ))
    }

    pub(crate) fn get_board_constructor(
        &self,
        model: &'model str,
    ) -> Result<&BoardConstructor, RegistryError> {
        if let Some(ctor) = self.board.get(model) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model.to_string()))
    }

    pub(crate) fn get_motor_constructor(
        &self,
        model: &'model str,
    ) -> Result<&MotorConstructor, RegistryError> {
        if let Some(ctor) = self.motors.get(model) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model.to_string()))
    }

    pub(crate) fn get_sensor_constructor(
        &self,
        model: &'model str,
    ) -> Result<&SensorConstructor, RegistryError> {
        if let Some(ctor) = self.sensor.get(model) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model.to_string()))
    }

    pub(crate) fn get_movement_sensor_constructor(
        &self,
        model: &'model str,
    ) -> Result<&MovementSensorConstructor, RegistryError> {
        if let Some(ctor) = self.movement_sensors.get(model) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model.to_string()))
    }

    pub(crate) fn get_encoder_constructor(
        &self,
        model: &'model str,
    ) -> Result<&EncoderConstructor, RegistryError> {
        if let Some(ctor) = self.encoders.get(model) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model.to_string()))
    }

    pub(crate) fn get_base_constructor(
        &self,
        model: &'model str,
    ) -> Result<&BaseConstructor, RegistryError> {
        if let Some(ctor) = self.bases.get(model) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::common;
    use crate::common::config::{ConfigType, StaticComponentConfig};
    use crate::common::registry::{ComponentRegistry, RegistryError};

    lazy_static::lazy_static! {
        static ref EMPTY_CONFIG: StaticComponentConfig = StaticComponentConfig::default();
    }

    #[test_log::test]
    fn test_registry() -> anyhow::Result<()> {
        let mut registry = ComponentRegistry::new();

        let ctor = registry.get_motor_constructor("fake".to_string());
        assert!(ctor.is_err());
        assert_eq!(ctor.err().unwrap(), RegistryError::ModelNotFound("fake"));
        common::motor::register_models(&mut registry);

        let ctor = registry.get_motor_constructor("fake".to_string());
        assert!(ctor.is_ok());

        let ret = registry.register_motor("fake", &|_, _| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            RegistryError::ModelAlreadyRegistered("fake".to_string())
        );

        let ret = registry.register_motor("fake2", &|_, _| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_ok());

        let ctor = registry.get_board_constructor("fake".to_string());
        assert!(ctor.is_err());
        assert_eq!(ctor.err().unwrap(), RegistryError::ModelNotFound("fake"));
        common::board::register_models(&mut registry);

        let ctor = registry.get_board_constructor("fake".to_string());
        assert!(ctor.is_ok());

        let ret = registry.register_board("fake", &|_| Err(anyhow::anyhow!("not implemented")));
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            RegistryError::ModelAlreadyRegistered("fake".to_string())
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
        let registry = ComponentRegistry::default();
        let ctor = registry.get_motor_constructor("fake".to_string());
        assert!(ctor.is_ok());

        let ctor = registry.get_board_constructor("fake".to_string());
        assert!(ctor.is_ok());
    }
}
