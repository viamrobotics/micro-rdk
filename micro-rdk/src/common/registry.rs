#![allow(dead_code)]
use std::collections::HashMap as Map;
use thiserror::Error;

use super::{
    base::{BaseError, BaseType},
    board::{BoardError, BoardType},
    config::ConfigType,
    encoder::{EncoderError, EncoderType},
    generic::{GenericComponentType, GenericError},
    motor::{MotorError, MotorType},
    movement_sensor::MovementSensorType,
    power_sensor::PowerSensorType,
    robot::Resource,
    sensor::{SensorError, SensorType},
    servo::{ServoError, ServoType},
};

#[cfg(feature = "camera")]
use super::camera::{CameraError, CameraType};
use crate::proto::common::v1::ResourceName;

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
    pub fn new(model: &str, name: String) -> Result<Self, RegistryError> {
        let model_str = match model {
            "motor" => crate::common::motor::COMPONENT_NAME,
            "board" => crate::common::board::COMPONENT_NAME,
            #[cfg(feature = "camera")]
            "camera" => crate::common::camera::COMPONENT_NAME,
            "encoder" => crate::common::encoder::COMPONENT_NAME,
            "movement_sensor" => crate::common::movement_sensor::COMPONENT_NAME,
            "sensor" => crate::common::sensor::COMPONENT_NAME,
            "base" => crate::common::base::COMPONENT_NAME,
            "servo" => crate::common::servo::COMPONENT_NAME,
            "power_sensor" => crate::common::power_sensor::COMPONENT_NAME,
            "generic" => crate::common::generic::COMPONENT_NAME,
            &_ => {
                return Err(RegistryError::ModelNotFound(model.to_string()));
            }
        };
        Ok(Self(model_str, name))
    }
}

impl TryFrom<ResourceName> for ResourceKey {
    type Error = RegistryError;
    fn try_from(value: ResourceName) -> Result<Self, Self::Error> {
        let comp_type: &str = &value.subtype;
        let comp_name = match comp_type {
            "motor" => crate::common::motor::COMPONENT_NAME,
            "sensor" => crate::common::sensor::COMPONENT_NAME,
            #[cfg(feature = "camera")]
            "camera" => crate::common::camera::COMPONENT_NAME,
            "movement_sensor" => crate::common::movement_sensor::COMPONENT_NAME,
            "encoder" => crate::common::encoder::COMPONENT_NAME,
            "base" => crate::common::base::COMPONENT_NAME,
            "servo" => crate::common::servo::COMPONENT_NAME,
            "power_sensor" => crate::common::power_sensor::COMPONENT_NAME,
            "generic" => crate::common::generic::COMPONENT_NAME,
            _ => {
                return Err(RegistryError::ModelNotFound(comp_type.to_string()));
            }
        };
        Ok(Self(comp_name, value.name))
    }
}

pub struct Dependency(pub ResourceKey, pub Resource);

/// Fn that returns a `BoardType`, `Arc<Mutex<dyn Board>>`
type BoardConstructor = dyn Fn(ConfigType) -> Result<BoardType, BoardError>;

/// Fn that returns a `MotorType`, `Arc<Mutex<dyn Motor>>`
type MotorConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> Result<MotorType, MotorError>;

/// Fn that returns a `SensorType`, `Arc<Mutex<dyn Sensor>>`
type SensorConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> Result<SensorType, SensorError>;

/// Fn that returns a `MovementSensorType`, `Arc<Mutex<dyn MovementSensor>>`
type MovementSensorConstructor =
    dyn Fn(ConfigType, Vec<Dependency>) -> Result<MovementSensorType, SensorError>;

/// Fn that returns an `EncoderType`, `Arc<Mutex<dyn Encoder>>`
type EncoderConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> Result<EncoderType, EncoderError>;

/// Fn that returns an `BaseType`, `Arc<Mutex<dyn Base>>`
type BaseConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> Result<BaseType, BaseError>;

/// Fn that returns a `CameraType`, `Arc<Mutex<dyn Camera>>`
#[cfg(feature = "camera")]
type CameraConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> Result<CameraType, CameraError>;

/// Fn that returns a `ServoType`, `Arc<Mutex<dyn Servo>>`
type ServoConstructor = dyn Fn(ConfigType, Vec<Dependency>) -> Result<ServoType, ServoError>;

/// Fn that returns a `PowerSensorType`, `Arc<Mutex<dyn PowerSensor>>`
type PowerSensorConstructor =
    dyn Fn(ConfigType, Vec<Dependency>) -> Result<PowerSensorType, SensorError>;

/// Fn that returns a `GenericComponentType`, `Arc<Mutex<dyn GenericComponentType>>`
type GenericComponentConstructor =
    dyn Fn(ConfigType, Vec<Dependency>) -> Result<GenericComponentType, GenericError>;

type DependenciesFromConfig = dyn Fn(ConfigType) -> Vec<ResourceKey>;

#[derive(Clone)]
pub struct ComponentRegistry {
    motors: Map<&'static str, &'static MotorConstructor>,
    board: Map<&'static str, &'static BoardConstructor>,
    #[cfg(feature = "camera")]
    camera: Map<&'static str, &'static CameraConstructor>,
    sensor: Map<&'static str, &'static SensorConstructor>,
    movement_sensors: Map<&'static str, &'static MovementSensorConstructor>,
    encoders: Map<&'static str, &'static EncoderConstructor>,
    bases: Map<&'static str, &'static BaseConstructor>,
    servos: Map<&'static str, &'static ServoConstructor>,
    power_sensors: Map<&'static str, &'static PowerSensorConstructor>,
    generic_components: Map<&'static str, &'static GenericComponentConstructor>,
    dependencies: Map<&'static str, Map<&'static str, &'static DependenciesFromConfig>>,
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        let mut r = Self::new();
        crate::common::board::register_models(&mut r);
        #[cfg(feature = "builtin-components")]
        {
            crate::common::encoder::register_models(&mut r);
            crate::common::motor::register_models(&mut r);
            crate::common::gpio_motor::register_models(&mut r);
            crate::common::gpio_servo::register_models(&mut r);
            crate::common::sensor::register_models(&mut r);
            crate::common::movement_sensor::register_models(&mut r);
            crate::common::mpu6050::register_models(&mut r);
            crate::common::adxl345::register_models(&mut r);
            crate::common::generic::register_models(&mut r);
            crate::common::ina::register_models(&mut r);
            crate::common::wheeled_base::register_models(&mut r);
            #[cfg(feature = "camera")]
            crate::common::camera::register_models(&mut r);
        }
        #[cfg(feature = "esp32")]
        {
            crate::esp32::board::register_models(&mut r);
            #[cfg(feature = "builtin-components")]
            {
                crate::esp32::encoder::register_models(&mut r);
                crate::esp32::hcsr04::register_models(&mut r);
                crate::esp32::single_encoder::register_models(&mut r);
            }
        }
        r
    }
}

impl ComponentRegistry {
    pub fn new() -> Self {
        let mut dependency_func_map = Map::new();
        dependency_func_map.insert(crate::common::motor::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::movement_sensor::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::encoder::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::sensor::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::base::COMPONENT_NAME, Map::new());
        #[cfg(feature = "camera")]
        dependency_func_map.insert(crate::common::camera::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::servo::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::power_sensor::COMPONENT_NAME, Map::new());
        dependency_func_map.insert(crate::common::generic::COMPONENT_NAME, Map::new());
        Self {
            motors: Map::new(),
            board: Map::new(),
            #[cfg(feature = "camera")]
            camera: Map::new(),
            sensor: Map::new(),
            movement_sensors: Map::new(),
            encoders: Map::new(),
            bases: Map::new(),
            servos: Map::new(),
            power_sensors: Map::new(),
            generic_components: Map::new(),
            dependencies: dependency_func_map,
        }
    }
    #[cfg(feature = "camera")]
    pub fn register_camera(
        &mut self,
        model: &'static str,
        constructor: &'static CameraConstructor,
    ) -> Result<(), RegistryError> {
        if self.camera.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.camera.insert(model, constructor);
        Ok(())
    }
    pub fn register_motor(
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

    pub fn register_sensor(
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

    pub fn register_movement_sensor(
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

    pub fn register_board(
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

    pub fn register_encoder(
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

    pub fn register_base(
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

    pub fn register_power_sensor(
        &mut self,
        model: &'static str,
        constructor: &'static PowerSensorConstructor,
    ) -> Result<(), RegistryError> {
        if self.power_sensors.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.power_sensors.insert(model, constructor);
        Ok(())
    }

    pub fn register_servo(
        &mut self,
        model: &'static str,
        constructor: &'static ServoConstructor,
    ) -> Result<(), RegistryError> {
        if self.servos.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.servos.insert(model, constructor);
        Ok(())
    }

    pub fn register_generic_component(
        &mut self,
        model: &'static str,
        constructor: &'static GenericComponentConstructor,
    ) -> Result<(), RegistryError> {
        if self.generic_components.contains_key(model) {
            return Err(RegistryError::ModelAlreadyRegistered(model));
        }
        let _ = self.generic_components.insert(model, constructor);
        Ok(())
    }

    pub fn register_dependency_getter(
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
        model_name: &str,
    ) -> Result<&'static DependenciesFromConfig, RegistryError> {
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
            model_name.to_owned(),
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

    #[cfg(feature = "camera")]
    pub(crate) fn get_camera_constructor(
        &self,
        model: String,
    ) -> Result<&'static CameraConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.camera.get(model_name) {
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

    pub(crate) fn get_power_sensor_constructor(
        &self,
        model: String,
    ) -> Result<&'static PowerSensorConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.power_sensors.get(model_name) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model))
    }

    pub(crate) fn get_servo_constructor(
        &self,
        model: String,
    ) -> Result<&'static ServoConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.servos.get(model_name) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model))
    }

    pub(crate) fn get_generic_component_constructor(
        &self,
        model: String,
    ) -> Result<&'static GenericComponentConstructor, RegistryError> {
        let model_name: &str = &model;
        if let Some(ctor) = self.generic_components.get(model_name) {
            return Ok(*ctor);
        }
        Err(RegistryError::ModelNotFound(model))
    }
}
#[cfg(test)]
mod tests {
    use crate::common::generic::DoCommand;
    use crate::common::motor::MotorError;
    use crate::google;

    use crate::common::sensor::SensorError;
    use crate::common::{
        self,
        config::{ConfigType, DynamicComponentConfig},
        registry::{ComponentRegistry, Dependency, RegistryError},
        robot::LocalRobot,
        sensor::{
            GenericReadingsResult, Readings, Sensor, SensorResult, SensorT, SensorType,
            TypedReadingsResult,
        },
        status::Status,
    };

    #[cfg(feature = "esp32")]
    use crate::esp32::exec::Esp32Executor;
    #[cfg(feature = "native")]
    use crate::native::exec::NativeExecutor;

    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    pub struct TestSensor {}

    impl TestSensor {
        pub fn new() -> Self {
            Self {}
        }
        pub fn from_config(
            _cfg: ConfigType,
            _: Vec<Dependency>,
        ) -> Result<SensorType, SensorError> {
            Ok(Arc::new(Mutex::new(Self {})))
        }
    }
    impl Default for TestSensor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Sensor for TestSensor {}

    impl Readings for TestSensor {
        fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
            Ok(self
                .get_readings()?
                .into_iter()
                .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
                .collect())
        }
    }

    impl SensorT<f64> for TestSensor {
        fn get_readings(&self) -> Result<TypedReadingsResult<f64>, SensorError> {
            let mut x = std::collections::HashMap::new();
            x.insert("test_sensor".to_string(), 42.0);
            Ok(x)
        }
    }

    impl Status for TestSensor {
        fn get_status(
            &self,
        ) -> Result<Option<google::protobuf::Struct>, crate::common::status::StatusError> {
            Ok(Some(google::protobuf::Struct {
                fields: HashMap::new(),
            }))
        }
    }

    impl DoCommand for TestSensor {}

    #[cfg(feature = "native")]
    type Executor = NativeExecutor;
    #[cfg(feature = "esp32")]
    type Executor = Esp32Executor;

    #[test_log::test]
    fn test_driver() {
        use crate::proto::app::v1::{ComponentConfig, ConfigResponse, RobotConfig};
        let components = vec![
            ComponentConfig {
                name: "board".to_string(),
                namespace: "rdk".to_string(),
                r#type: "board".to_string(),
                model: "rdk:builtin:fake".to_string(),
                attributes: None,
                ..Default::default()
            },
            ComponentConfig {
                name: "test_sensor".to_string(),
                namespace: "rdk".to_string(),
                r#type: "sensor".to_string(),
                model: "rdk:builtin:test_sensor".to_string(),
                attributes: None,
                ..Default::default()
            },
        ];

        let config: Option<RobotConfig> = Some(RobotConfig {
            components,
            ..Default::default()
        });

        let cfg_resp = ConfigResponse { config };
        let mut registry = ComponentRegistry::new();

        // sensor should not be registered yet
        let ctor = registry.get_sensor_constructor("test_sensor".to_string());
        assert!(ctor.is_err());
        assert_eq!(
            ctor.err().unwrap(),
            RegistryError::ModelNotFound("test_sensor".to_string())
        );

        // register fake board
        common::board::register_models(&mut registry);
        let ctor = registry.get_board_constructor("fake".to_string());
        assert!(ctor.is_ok());

        // register test sensor
        assert!(registry
            .register_sensor("test_sensor", &TestSensor::from_config)
            .is_ok());

        // check ctor
        let ctor = registry.get_sensor_constructor("test_sensor".to_string());
        assert!(ctor.is_ok());

        // make robot
        let robot = LocalRobot::from_cloud_config(
            Executor::new(),
            "".to_string(),
            &cfg_resp,
            Box::new(registry),
            None,
        );
        assert!(robot.is_ok());
        let robot = robot.unwrap();

        // get test value from sensor
        let test_sensor = robot
            .get_sensor_by_name("test_sensor".to_string())
            .expect("could not find test_sensor");
        let r = test_sensor
            .lock()
            .unwrap()
            .get_generic_readings()
            .unwrap()
            .get("test_sensor")
            .expect("could not get reading")
            .clone();
        assert_eq!(
            r,
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(42.0))
            }
        );
    }

    #[test_log::test]
    fn test_registry() {
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

        let ret = registry.register_motor("fake", &|_, _| {
            Err(MotorError::MotorMethodUnimplemented(""))
        });
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            RegistryError::ModelAlreadyRegistered("fake")
        );

        let ret = registry.register_motor("fake2", &|_, _| {
            Err(MotorError::MotorMethodUnimplemented(""))
        });
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

        let ret = registry.register_board("fake", &|_| {
            Err(common::board::BoardError::BoardMethodNotSupported(""))
        });
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            RegistryError::ModelAlreadyRegistered("fake")
        );

        let ret = registry.register_board("fake2", &|_| {
            Err(common::board::BoardError::BoardMethodNotSupported(""))
        });
        assert!(ret.is_ok());

        let ctor = registry.get_motor_constructor("fake2".to_string());
        assert!(ctor.is_ok());

        let ret = ctor.unwrap()(
            ConfigType::Dynamic(&DynamicComponentConfig::default()),
            Vec::new(),
        );

        assert!(ret.is_err());
        assert_eq!(format!("{}", ret.err().unwrap()), "unimplemented: ");

        let ctor = registry.get_board_constructor("fake2".to_string());
        assert!(ctor.is_ok());

        let ret = ctor.unwrap()(ConfigType::Dynamic(&DynamicComponentConfig::default()));

        assert!(ret.is_err());
        assert_eq!(format!("{}", ret.err().unwrap()), "method:  not supported");
    }
}
