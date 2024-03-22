#![allow(dead_code)]

use chrono::{DateTime, FixedOffset};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(feature = "camera")]
use crate::camera::{Camera, CameraType};

use crate::{
    common::actuator::Actuator,
    common::base::Base,
    common::board::Board,
    common::encoder::Encoder,
    common::motor::Motor,
    common::movement_sensor::MovementSensor,
    common::sensor::Sensor,
    common::status::Status,
    google,
    proto::{
        app::v1::ConfigResponse,
        common::{self, v1::ResourceName},
        robot,
    },
};
use log::*;

use super::{
    actuator::ActuatorError,
    base::BaseType,
    board::BoardType,
    config::{AttributeError, Component, ConfigType, DynamicComponentConfig},
    encoder::EncoderType,
    generic::{GenericComponent, GenericComponentType},
    motor::MotorType,
    movement_sensor::MovementSensorType,
    power_sensor::{PowerSensor, PowerSensorType},
    registry::{
        get_board_from_dependencies, ComponentRegistry, Dependency, RegistryError, ResourceKey,
    },
    sensor::SensorType,
    servo::{Servo, ServoType},
};
use thiserror::Error;

static NAMESPACE_PREFIX: &str = "rdk:builtin:";

#[derive(Clone)]
pub enum ResourceType {
    Motor(MotorType),
    Board(BoardType),
    Base(BaseType),
    Sensor(SensorType),
    MovementSensor(MovementSensorType),
    Encoder(EncoderType),
    PowerSensor(PowerSensorType),
    Servo(ServoType),
    Generic(GenericComponentType),
    #[cfg(feature = "camera")]
    Camera(CameraType),
}
pub type Resource = ResourceType;
pub type ResourceMap = HashMap<ResourceName, Resource>;

#[cfg(feature = "data")]
impl ResourceType {
    pub fn component_type(&self) -> String {
        match self {
            Self::Base(_) => "rdk:component:base",
            Self::Board(_) => "rdk:component:board",
            Self::Encoder(_) => "rdk:component:encoder",
            Self::Generic(_) => "rdk:component:generic",
            Self::Motor(_) => "rdk:component:motor",
            Self::MovementSensor(_) => "rdk:component:movement_sensor",
            Self::PowerSensor(_) => "rdk:component:power_sensor",
            Self::Sensor(_) => "rdk:component:sensor",
            Self::Servo(_) => "rdk:component:servo",
        }
        .to_string()
    }
}

#[derive(Default)]
pub struct LocalRobot {
    resources: ResourceMap,
    build_time: Option<DateTime<FixedOffset>>,
}

#[derive(Error, Debug)]
pub enum RobotError {
    #[error("no board setup")]
    RobotNoBoard,
    #[error("{0} type is not supported")]
    RobotComponentTypeNotSupported(String),
    #[error("wrong model prefix {0} expected 'rdk:builtin'")]
    RobotModelWrongPrefix(String),
    #[error("model is missing")]
    RobotModelAbsent,
    #[error(transparent)]
    RobotRegistryError(#[from] RegistryError),
    #[error("missing {0} dependency for {1}")]
    RobotDependencyMissing(String, String),
    #[error(transparent)]
    RobotResourceBuildError(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    RobotParseConfigError(#[from] AttributeError),
    #[error(transparent)]
    RobotActuatorError(#[from] ActuatorError),
}

fn resource_name_from_component_cfg(cfg: &DynamicComponentConfig) -> ResourceName {
    ResourceName {
        namespace: cfg.namespace.to_string(),
        r#type: "component".to_string(),
        subtype: cfg.r#type.to_string(),
        name: cfg.name.to_string(),
    }
}

// Extracts model string from the full namespace provided by incoming instances of ComponentConfig.
// TODO: This prefix requirement was put in place due to model names sent from app being otherwise
// prefixed with "rdk:builtin:". A more ideal and robust method of namespacing is preferred.
fn get_model_without_namespace_prefix(full_model: &mut String) -> Result<String, RobotError> {
    if !full_model.starts_with(NAMESPACE_PREFIX) {
        return Err(RobotError::RobotModelWrongPrefix(full_model.to_string()));
    }
    let model = full_model.split_off(NAMESPACE_PREFIX.len());
    if model.is_empty() {
        return Err(RobotError::RobotModelAbsent);
    }
    Ok(model)
}

impl LocalRobot {
    pub fn new() -> Self {
        Default::default()
    }
    // Inserts components in order of dependency. If a component's dependencies are not satisfied it is
    // temporarily skipped and sent to the end of the queue. This process repeats until all the components
    // are added (or a max number of iterations are reached, indicating a configuration error). We have not
    // selected the most time-efficient algorithm for solving this problem in order to minimize memory usage
    fn process_components(
        &mut self,
        mut components: Vec<Option<DynamicComponentConfig>>,
        mut registry: Box<ComponentRegistry>,
    ) -> Result<(), RobotError> {
        let config = components
            .iter_mut()
            .find(|cfg| cfg.as_ref().map_or(false, |cfg| cfg.r#type == "board"));
        let (board, board_key) = if let Some(Some(config)) = config {
            let model = get_model_without_namespace_prefix(&mut config.model.to_owned())?;
            let board_key = Some(ResourceKey(
                crate::common::board::COMPONENT_NAME,
                config.name.to_string(),
            ));
            let constructor = registry
                .get_board_constructor(model)
                .map_err(RobotError::RobotRegistryError)?;
            let board = constructor(ConfigType::Dynamic(config))
                .map_err(|e| RobotError::RobotResourceBuildError(e.into()))?;
            (Some(board), board_key)
        } else {
            (None, None)
        };
        let mut resource_to_build = components.len();
        let max_iteration = resource_to_build * 2;
        let mut num_iteration = 0;
        let mut iter = (0..resource_to_build).cycle();
        while resource_to_build > 0 && num_iteration < max_iteration {
            num_iteration += 1;
            let cfg = &mut components[iter.next().unwrap()];
            if let Some(cfg) = cfg.as_ref() {
                // capture the error and make it available to LocalRobot so it can be pushed in the logs?
                if self
                    .build_resource(cfg, board.clone(), board_key.clone(), &mut registry)
                    .is_err()
                {
                    continue;
                }
            } else {
                continue;
            }
            let _ = cfg.take();
            resource_to_build -= 1;
        }
        if resource_to_build > 0 {
            log::error!(
                "These components couldn't be built {:?}. Check for errors, missing or circular dependencies in the config.",
                components
                    .into_iter()
                    .flatten()
                    .map(|x| x.name)
                    .collect::<Vec<String>>()
            )
        }
        Ok(())
    }

    // Creates a robot from the response of a gRPC call to acquire the robot configuration. The individual
    // component configs within the response are consumed and the corresponding components are generated
    // and added to the created robot.
    pub fn from_cloud_config(
        config_resp: &ConfigResponse,
        registry: Box<ComponentRegistry>,
        build_time: Option<DateTime<FixedOffset>>,
    ) -> anyhow::Result<Self> {
        let mut robot = LocalRobot {
            resources: ResourceMap::new(),
            // Use date time pulled off gRPC header as the `build_time` returned in the status of
            // every resource as `last_reconfigured`.
            build_time,
        };

        let components: Result<Vec<Option<DynamicComponentConfig>>, AttributeError> = config_resp
            .config
            .as_ref()
            .unwrap()
            .components
            .iter()
            .map(|x| x.try_into().map(Option::Some))
            .collect();
        robot.process_components(
            components.map_err(RobotError::RobotParseConfigError)?,
            registry,
        )?;
        Ok(robot)
    }

    fn build_resource(
        &mut self,
        config: &DynamicComponentConfig,
        board: Option<BoardType>,
        board_name: Option<ResourceKey>,
        registry: &mut ComponentRegistry,
    ) -> Result<(), RobotError> {
        let new_resource_name = resource_name_from_component_cfg(config);
        let model = get_model_without_namespace_prefix(&mut config.get_model().to_owned())?;

        let mut dependencies = self.get_config_dependencies(config, registry)?;

        if let Some(b) = board.as_ref() {
            dependencies.push(Dependency(
                board_name.as_ref().unwrap().clone(),
                ResourceType::Board(b.clone()),
            ));
        }
        self.insert_resource(
            model,
            new_resource_name,
            ConfigType::Dynamic(config),
            dependencies,
            registry,
        )?;
        Ok(())
    }

    fn get_config_dependencies(
        &mut self,
        config: &DynamicComponentConfig,
        registry: &mut ComponentRegistry,
    ) -> Result<Vec<Dependency>, RobotError> {
        let type_as_static = match config.get_type() {
            "motor" => crate::common::motor::COMPONENT_NAME,
            "board" => crate::common::board::COMPONENT_NAME,
            "encoder" => crate::common::encoder::COMPONENT_NAME,
            "movement_sensor" => crate::common::movement_sensor::COMPONENT_NAME,
            "sensor" => crate::common::sensor::COMPONENT_NAME,
            "base" => crate::common::base::COMPONENT_NAME,
            "power_sensor" => crate::common::power_sensor::COMPONENT_NAME,
            "servo" => crate::common::servo::COMPONENT_NAME,
            "generic" => crate::common::generic::COMPONENT_NAME,
            &_ => {
                return Err(RobotError::RobotComponentTypeNotSupported(
                    config.get_type().to_owned(),
                ))
            }
        };
        let model = get_model_without_namespace_prefix(&mut config.get_model().to_owned())?;
        let deps_keys = registry
            .get_dependency_function(type_as_static, &model)
            .map_or(Vec::new(), |dep_fn| dep_fn(ConfigType::Dynamic(config)));

        deps_keys
            .into_iter()
            .map(|key| {
                let r_name = ResourceName {
                    namespace: config.namespace.clone(),
                    r#type: "component".to_owned(),
                    subtype: key.0.to_owned(),
                    name: key.1.clone(),
                };

                let res = match self.resources.get(&r_name) {
                    Some(r) => r.clone(),
                    None => {
                        return Err(RobotError::RobotDependencyMissing(
                            key.1,
                            config.name.to_owned(),
                        ));
                    }
                };
                Ok(Dependency(ResourceKey(key.0, key.1.clone()), res))
            })
            .collect()
    }

    fn insert_resource(
        &mut self,
        model: String,
        r_name: ResourceName,
        cfg: ConfigType,
        deps: Vec<Dependency>,
        registry: &mut ComponentRegistry,
    ) -> Result<(), RobotError> {
        let r_type = cfg.get_type();
        let res = match r_type {
            "motor" => {
                let ctor = registry
                    .get_motor_constructor(model)
                    .map_err(RobotError::RobotRegistryError)?;
                ResourceType::Motor(
                    ctor(cfg, deps).map_err(|e| RobotError::RobotResourceBuildError(e.into()))?,
                )
            }
            "board" => {
                let board = get_board_from_dependencies(deps);
                ResourceType::Board(match board {
                    Some(b) => b.clone(),
                    None => return Ok(()),
                })
            }
            "sensor" => {
                let ctor = registry
                    .get_sensor_constructor(model)
                    .map_err(RobotError::RobotRegistryError)?;
                ResourceType::Sensor(
                    ctor(cfg, deps).map_err(|e| RobotError::RobotResourceBuildError(e.into()))?,
                )
            }
            "movement_sensor" => {
                let ctor = registry
                    .get_movement_sensor_constructor(model)
                    .map_err(RobotError::RobotRegistryError)?;
                ResourceType::MovementSensor(
                    ctor(cfg, deps).map_err(|e| RobotError::RobotResourceBuildError(e.into()))?,
                )
            }
            "encoder" => {
                let ctor = registry
                    .get_encoder_constructor(model)
                    .map_err(RobotError::RobotRegistryError)?;
                ResourceType::Encoder(
                    ctor(cfg, deps).map_err(|e| RobotError::RobotResourceBuildError(e.into()))?,
                )
            }
            "base" => {
                let ctor = registry
                    .get_base_constructor(model)
                    .map_err(RobotError::RobotRegistryError)?;
                ResourceType::Base(
                    ctor(cfg, deps).map_err(|e| RobotError::RobotResourceBuildError(e.into()))?,
                )
            }
            "power_sensor" => {
                let ctor = registry
                    .get_power_sensor_constructor(model)
                    .map_err(RobotError::RobotRegistryError)?;
                ResourceType::PowerSensor(
                    ctor(cfg, deps).map_err(|e| RobotError::RobotResourceBuildError(e.into()))?,
                )
            }
            "servo" => {
                let ctor = registry
                    .get_servo_constructor(model)
                    .map_err(RobotError::RobotRegistryError)?;
                ResourceType::Servo(
                    ctor(cfg, deps).map_err(|e| RobotError::RobotResourceBuildError(e.into()))?,
                )
            }
            "generic" => {
                let ctor = registry
                    .get_generic_component_constructor(model)
                    .map_err(RobotError::RobotRegistryError)?;
                ResourceType::Generic(
                    ctor(cfg, deps).map_err(|e| RobotError::RobotResourceBuildError(e.into()))?,
                )
            }
            &_ => {
                return Err(RobotError::RobotComponentTypeNotSupported(
                    r_type.to_owned(),
                ));
            }
        };
        self.resources.insert(r_name, res);
        Ok(())
    }

    pub fn get_status(
        &mut self,
        mut msg: robot::v1::GetStatusRequest,
    ) -> anyhow::Result<Vec<robot::v1::Status>> {
        let last_reconfigured_proto = self.build_time.map(|bt| google::protobuf::Timestamp {
            seconds: bt.timestamp(),
            nanos: bt.timestamp_subsec_nanos() as i32,
        });
        if msg.resource_names.is_empty() {
            let mut vec = Vec::with_capacity(self.resources.len());
            for (name, val) in self.resources.iter_mut() {
                match val {
                    ResourceType::Motor(m) => {
                        let status = m.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            last_reconfigured: last_reconfigured_proto.clone(),
                            status,
                        });
                    }
                    ResourceType::Board(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            last_reconfigured: last_reconfigured_proto.clone(),
                            status,
                        });
                    }
                    ResourceType::Base(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            last_reconfigured: last_reconfigured_proto.clone(),
                            status,
                        });
                    }
                    ResourceType::Sensor(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            last_reconfigured: last_reconfigured_proto.clone(),
                            status,
                        });
                    }
                    ResourceType::MovementSensor(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            last_reconfigured: last_reconfigured_proto.clone(),
                            status,
                        });
                    }
                    ResourceType::Encoder(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            last_reconfigured: last_reconfigured_proto.clone(),
                            status,
                        });
                    }
                    ResourceType::PowerSensor(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            last_reconfigured: last_reconfigured_proto.clone(),
                            status,
                        });
                    }
                    ResourceType::Servo(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            last_reconfigured: last_reconfigured_proto.clone(),
                            status,
                        });
                    }
                    ResourceType::Generic(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            last_reconfigured: last_reconfigured_proto.clone(),
                            status,
                        });
                    }
                    #[cfg(feature = "camera")]
                    _ => continue,
                };
            }
            return Ok(vec);
        }
        let mut vec = Vec::with_capacity(msg.resource_names.len());
        for name in msg.resource_names.drain(0..) {
            debug!("processing {:?}", name);
            match self.resources.get_mut(&name) {
                Some(val) => {
                    match val {
                        ResourceType::Motor(m) => {
                            let status = m.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                last_reconfigured: last_reconfigured_proto.clone(),
                                status,
                            });
                        }
                        ResourceType::Board(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                last_reconfigured: last_reconfigured_proto.clone(),
                                status,
                            });
                        }
                        ResourceType::Base(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                last_reconfigured: last_reconfigured_proto.clone(),
                                status,
                            });
                        }
                        ResourceType::Sensor(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                last_reconfigured: last_reconfigured_proto.clone(),
                                status,
                            });
                        }
                        ResourceType::MovementSensor(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                last_reconfigured: last_reconfigured_proto.clone(),
                                status,
                            });
                        }
                        ResourceType::Encoder(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                last_reconfigured: last_reconfigured_proto.clone(),
                                status,
                            });
                        }
                        ResourceType::PowerSensor(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                last_reconfigured: last_reconfigured_proto.clone(),
                                status,
                            });
                        }
                        ResourceType::Servo(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                last_reconfigured: last_reconfigured_proto.clone(),
                                status,
                            });
                        }
                        ResourceType::Generic(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                last_reconfigured: last_reconfigured_proto.clone(),
                                status,
                            });
                        }
                        #[cfg(feature = "camera")]
                        _ => continue,
                    };
                }
                None => continue,
            };
        }
        Ok(vec)
    }
    pub fn get_resource_names(&self) -> anyhow::Result<Vec<common::v1::ResourceName>> {
        let mut name = Vec::with_capacity(self.resources.len());
        for k in self.resources.keys() {
            name.push(k.clone());
        }
        Ok(name)
    }
    pub fn get_motor_by_name(&self, name: String) -> Option<Arc<Mutex<dyn Motor>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "motor".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::Motor(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }
    #[cfg(feature = "camera")]
    pub fn get_camera_by_name(&self, name: String) -> Option<Arc<Mutex<dyn Camera>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "camera".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::Camera(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }
    pub fn get_base_by_name(&self, name: String) -> Option<Arc<Mutex<dyn Base>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "base".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::Base(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }
    pub fn get_board_by_name(&self, name: String) -> Option<Arc<Mutex<dyn Board>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "board".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::Board(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }
    pub fn get_sensor_by_name(&self, name: String) -> Option<Arc<Mutex<dyn Sensor>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "sensor".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::Sensor(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }

    pub fn get_movement_sensor_by_name(
        &self,
        name: String,
    ) -> Option<Arc<Mutex<dyn MovementSensor>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "movement_sensor".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::MovementSensor(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }

    pub fn get_encoder_by_name(&self, name: String) -> Option<Arc<Mutex<dyn Encoder>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "encoder".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::Encoder(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }

    pub fn get_power_sensor_by_name(&self, name: String) -> Option<Arc<Mutex<dyn PowerSensor>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "power_sensor".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::PowerSensor(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }

    pub fn get_servo_by_name(&self, name: String) -> Option<Arc<Mutex<dyn Servo>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "servo".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::Servo(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }

    pub fn get_generic_component_by_name(
        &self,
        name: String,
    ) -> Option<Arc<Mutex<dyn GenericComponent>>> {
        let name = ResourceName {
            namespace: "rdk".to_string(),
            r#type: "component".to_string(),
            subtype: "generic".to_string(),
            name,
        };
        match self.resources.get(&name) {
            Some(ResourceType::Generic(r)) => Some(r.clone()),
            Some(_) => None,
            None => None,
        }
    }

    pub fn stop_all(&mut self) -> Result<(), RobotError> {
        let mut stop_errors: Vec<ActuatorError> = vec![];
        for resource in self.resources.values_mut() {
            match resource {
                ResourceType::Base(b) => {
                    match b.stop() {
                        Ok(_) => {}
                        Err(err) => {
                            stop_errors.push(err);
                        }
                    };
                }
                ResourceType::Motor(m) => {
                    match m.stop() {
                        Ok(_) => {}
                        Err(err) => {
                            stop_errors.push(err);
                        }
                    };
                }
                _ => continue,
            }
        }
        if !stop_errors.is_empty() {
            return Err(RobotError::RobotActuatorError(stop_errors.pop().unwrap()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::analog::AnalogReader;
    use crate::common::board::Board;
    use crate::common::config::{DynamicComponentConfig, Kind};
    use crate::common::encoder::{Encoder, EncoderPositionType};
    use crate::common::i2c::I2CHandle;
    use crate::common::motor::Motor;
    use crate::common::movement_sensor::MovementSensor;
    use crate::common::robot::LocalRobot;
    use crate::common::sensor::Readings;
    use crate::google;
    use crate::google::protobuf::Struct;
    use crate::proto::app::v1::{ComponentConfig, ConfigResponse, RobotConfig};

    #[test_log::test]
    fn test_robot_from_components() {
        let robot_config: Vec<Option<DynamicComponentConfig>> = vec![
            Some(DynamicComponentConfig {
                name: "board".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "board".to_owned(),
                model: "rdk:builtin:fake".to_owned(),
                attributes: Some(HashMap::from([
                    (
                        "pins".to_owned(),
                        Kind::VecValue(vec![
                            Kind::StringValue("11".to_owned()),
                            Kind::StringValue("12".to_owned()),
                            Kind::StringValue("13".to_owned()),
                        ]),
                    ),
                    (
                        "analogs".to_owned(),
                        Kind::StructValue(HashMap::from([(
                            "1".to_owned(),
                            Kind::StringValue("11.12".to_owned()),
                        )])),
                    ),
                    (
                        "i2cs".to_owned(),
                        Kind::VecValue(vec![
                            Kind::StructValue(HashMap::from([(
                                "name".to_owned(),
                                Kind::StringValue("i2c0".to_owned()),
                            )])),
                            Kind::StructValue(HashMap::from([
                                ("name".to_owned(), Kind::StringValue("i2c1".to_owned())),
                                ("value_1".to_owned(), Kind::StringValue("5".to_owned())),
                                ("value_2".to_owned(), Kind::StringValue("4".to_owned())),
                            ])),
                        ]),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "motor".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "rdk:builtin:fake".to_owned(),
                attributes: Some(HashMap::from([
                    ("max_rpm".to_owned(), Kind::StringValue("100".to_owned())),
                    (
                        "fake_position".to_owned(),
                        Kind::StringValue("1205".to_owned()),
                    ),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([
                            ("a".to_owned(), Kind::StringValue("29".to_owned())),
                            ("b".to_owned(), Kind::StringValue("5".to_owned())),
                            ("pwm".to_owned(), Kind::StringValue("12".to_owned())),
                        ])),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "sensor".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "sensor".to_owned(),
                model: "rdk:builtin:fake".to_owned(),
                attributes: Some(HashMap::from([(
                    "fake_value".to_owned(),
                    Kind::StringValue("11.12".to_owned()),
                )])),
            }),
            Some(DynamicComponentConfig {
                name: "m_sensor".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "movement_sensor".to_owned(),
                model: "rdk:builtin:fake".to_owned(),
                attributes: Some(HashMap::from([
                    ("fake_lat".to_owned(), Kind::StringValue("68.86".to_owned())),
                    (
                        "fake_lon".to_owned(),
                        Kind::StringValue("-85.44".to_owned()),
                    ),
                    (
                        "fake_alt".to_owned(),
                        Kind::StringValue("3000.1".to_owned()),
                    ),
                    (
                        "lin_acc_x".to_owned(),
                        Kind::StringValue("200.2".to_owned()),
                    ),
                    (
                        "lin_acc_y".to_owned(),
                        Kind::StringValue("-100.3".to_owned()),
                    ),
                    (
                        "lin_acc_z".to_owned(),
                        Kind::StringValue("100.4".to_owned()),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "enc1".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "encoder".to_owned(),
                model: "rdk:builtin:fake".to_owned(),
                attributes: Some(HashMap::from([
                    ("fake_deg".to_owned(), Kind::StringValue("45.0".to_owned())),
                    (
                        "ticks_per_rotation".to_owned(),
                        Kind::StringValue("2".to_owned()),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "enc2".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "encoder".to_owned(),
                model: "rdk:builtin:fake_incremental".to_owned(),
                attributes: Some(HashMap::from([(
                    "fake_ticks".to_owned(),
                    Kind::StringValue("3.0".to_owned()),
                )])),
            }),
        ];

        let mut robot = LocalRobot::default();

        let ret = robot.process_components(robot_config, Box::default());
        ret.unwrap();

        let motor = robot.get_motor_by_name("motor".to_string());

        assert!(motor.is_some());

        let position = motor.unwrap().get_position();

        assert!(position.is_ok());

        assert_eq!(position.ok().unwrap(), 1205);

        let board = robot.get_board_by_name("board".to_string());

        assert!(board.is_some());

        assert!(board
            .as_ref()
            .unwrap()
            .get_analog_reader_by_name("1".to_string())
            .is_ok());

        let value = board
            .as_ref()
            .unwrap()
            .get_analog_reader_by_name("1".to_string())
            .unwrap()
            .clone()
            .read();

        assert!(value.is_ok());

        assert_eq!(value.unwrap(), 11);

        let mut i2c_driver = board.as_ref().unwrap().get_i2c_by_name("i2c0".to_string());
        assert!(i2c_driver.is_ok());
        let bytes: [u8; 3] = [0, 1, 2];
        assert!(i2c_driver.as_mut().unwrap().write_i2c(0, &bytes).is_ok());
        let mut buffer: [u8; 3] = [0, 0, 0];
        assert!(i2c_driver
            .as_mut()
            .unwrap()
            .read_i2c(0, &mut buffer)
            .is_ok());
        assert!(buffer.iter().zip(bytes.iter()).all(|(a, b)| a == b));

        let mut i2c_driver_2 = board.as_ref().unwrap().get_i2c_by_name("i2c1".to_string());
        assert!(i2c_driver_2.is_ok());
        let init_bytes: [u8; 3] = [5, 4, 0];
        let mut buffer_2: [u8; 3] = [0, 0, 0];
        assert!(i2c_driver_2
            .as_mut()
            .unwrap()
            .read_i2c(0, &mut buffer_2)
            .is_ok());
        assert!(buffer_2.iter().zip(init_bytes.iter()).all(|(a, b)| a == b));

        let sensor = robot.get_sensor_by_name("sensor".to_string());

        assert!(sensor.is_some());

        let value = sensor.unwrap().get_generic_readings();

        assert!(value.is_ok());

        assert!(value.as_ref().unwrap().contains_key("fake_sensor"));

        let value = value
            .as_ref()
            .unwrap()
            .get("fake_sensor")
            .unwrap()
            .kind
            .clone();

        assert!(value.is_some());

        let value = match value {
            Some(google::protobuf::value::Kind::NumberValue(a)) => Some(a),
            _ => None,
        };

        assert!(value.is_some());

        assert_eq!(value.unwrap(), 11.12);

        let m_sensor = robot.get_movement_sensor_by_name("m_sensor".to_string());

        assert!(m_sensor.is_some());

        let m_sensor_pos = m_sensor.unwrap().get_position();

        assert!(m_sensor_pos.is_ok());

        let unwrapped_pos = m_sensor_pos.unwrap();

        assert_eq!(unwrapped_pos.lat, 68.86);
        assert_eq!(unwrapped_pos.lon, -85.44);
        assert_eq!(unwrapped_pos.alt, 3000.1);

        let m_sensor_2 = robot.get_movement_sensor_by_name("m_sensor".to_string());

        assert!(m_sensor_2.is_some());

        let lin_acc_result = m_sensor_2.unwrap().get_linear_acceleration();
        assert!(lin_acc_result.is_ok());
        let lin_acc = lin_acc_result.unwrap();
        assert_eq!(lin_acc.x, 200.2);
        assert_eq!(lin_acc.y, -100.3);
        assert_eq!(lin_acc.z, 100.4);

        let mut enc1 = robot.get_encoder_by_name("enc1".to_string());
        assert!(enc1.is_some());

        let props = enc1.as_mut().unwrap().get_properties();
        assert!(props.ticks_count_supported);
        assert!(props.angle_degrees_supported);

        let pos_deg = enc1
            .as_mut()
            .unwrap()
            .get_position(EncoderPositionType::DEGREES);
        assert!(pos_deg.is_ok());
        assert_eq!(
            pos_deg.as_ref().unwrap().position_type,
            EncoderPositionType::DEGREES
        );
        assert_eq!(pos_deg.as_ref().unwrap().value, 45.0);

        let pos_tick = enc1
            .as_mut()
            .unwrap()
            .get_position(EncoderPositionType::TICKS);
        assert!(pos_tick.is_ok());
        assert_eq!(pos_tick.as_ref().unwrap().value, 0.25);
        assert_eq!(
            pos_tick.as_ref().unwrap().position_type,
            EncoderPositionType::TICKS
        );

        let mut enc2 = robot.get_encoder_by_name("enc2".to_string());
        assert!(enc2.is_some());

        let pos_deg = enc2
            .as_mut()
            .unwrap()
            .get_position(EncoderPositionType::DEGREES);
        assert!(pos_deg.is_err());

        let pos_tick = enc2
            .as_mut()
            .unwrap()
            .get_position(EncoderPositionType::TICKS);
        assert!(pos_tick.is_ok());
        assert_eq!(
            pos_tick.as_ref().unwrap().position_type,
            EncoderPositionType::TICKS
        );
        assert_eq!(pos_tick.as_ref().unwrap().value, 3.0);

        let pos_deg = enc2
            .as_mut()
            .unwrap()
            .get_position(EncoderPositionType::DEGREES);
        assert!(pos_deg.is_err());
    }

    #[test_log::test]
    fn test_from_cloud_config() {
        let mut component_cfgs = Vec::new();

        let comp = ComponentConfig {
            name: "enc1".to_string(),
            model: "rdk:builtin:fake".to_string(),
            r#type: "encoder".to_string(),
            namespace: "rdk".to_string(),
            frame: None,
            depends_on: Vec::new(),
            service_configs: Vec::new(),
            api: "blah".to_string(),
            attributes: Some(Struct {
                fields: HashMap::from([(
                    "fake_deg".to_string(),
                    google::protobuf::Value {
                        kind: Some(google::protobuf::value::Kind::NumberValue(90.0)),
                    },
                )]),
            }),
            ..Default::default()
        };
        component_cfgs.push(comp);

        let comp2 = ComponentConfig {
            name: "m1".to_string(),
            model: "rdk:builtin:fake_with_dep".to_string(),
            r#type: "motor".to_string(),
            namespace: "rdk".to_string(),
            frame: None,
            depends_on: Vec::new(),
            service_configs: Vec::new(),
            api: "blah".to_string(),
            attributes: Some(Struct {
                fields: HashMap::from([(
                    "encoder".to_string(),
                    google::protobuf::Value {
                        kind: Some(google::protobuf::value::Kind::StringValue(
                            "enc1".to_string(),
                        )),
                    },
                )]),
            }),
            ..Default::default()
        };
        component_cfgs.push(comp2);

        let comp3: ComponentConfig = ComponentConfig {
            name: "m2".to_string(),
            model: "rdk:builtin:fake_with_dep".to_string(),
            r#type: "motor".to_string(),
            namespace: "rdk".to_string(),
            frame: None,
            depends_on: Vec::new(),
            service_configs: Vec::new(),
            api: "blah".to_string(),
            attributes: Some(Struct {
                fields: HashMap::from([(
                    "encoder".to_string(),
                    google::protobuf::Value {
                        kind: Some(google::protobuf::value::Kind::StringValue(
                            "enc2".to_string(),
                        )),
                    },
                )]),
            }),
            ..Default::default()
        };
        component_cfgs.push(comp3);

        let comp4 = ComponentConfig {
            name: "enc2".to_string(),
            model: "rdk:builtin:fake".to_string(),
            r#type: "encoder".to_string(),
            namespace: "rdk".to_string(),
            frame: None,
            depends_on: Vec::new(),
            service_configs: Vec::new(),
            api: "blah".to_string(),
            attributes: Some(Struct {
                fields: HashMap::from([(
                    "fake_deg".to_string(),
                    google::protobuf::Value {
                        kind: Some(google::protobuf::value::Kind::NumberValue(180.0)),
                    },
                )]),
            }),
            ..Default::default()
        };
        component_cfgs.push(comp4);

        let robot_cfg = ConfigResponse {
            config: Some(RobotConfig {
                components: component_cfgs,
                ..Default::default()
            }),
        };

        let robot = LocalRobot::from_cloud_config(&robot_cfg, Box::default(), None);

        assert!(robot.is_ok());

        let robot = robot.unwrap();

        let m1 = robot.get_motor_by_name("m1".to_string());

        assert!(m1.is_some());

        let position = m1.unwrap().get_position();

        assert!(position.is_ok());

        assert_eq!(position.ok().unwrap(), 90);

        let m2 = robot.get_motor_by_name("m2".to_string());

        assert!(m2.is_some());

        let position = m2.unwrap().get_position();

        assert!(position.is_ok());

        assert_eq!(position.ok().unwrap(), 180);
    }

    #[test_log::test]
    fn test_cloud_config_missing_dependencies() {
        let mut component_cfgs = Vec::new();

        let comp2 = ComponentConfig {
            name: "m1".to_string(),
            model: "rdk:builtin:fake_with_dep".to_string(),
            r#type: "motor".to_string(),
            namespace: "rdk".to_string(),
            frame: None,
            depends_on: Vec::new(),
            service_configs: Vec::new(),
            api: "blah".to_string(),
            attributes: Some(Struct {
                fields: HashMap::from([(
                    "encoder".to_string(),
                    google::protobuf::Value {
                        kind: Some(google::protobuf::value::Kind::StringValue(
                            "enc1".to_string(),
                        )),
                    },
                )]),
            }),
            ..Default::default()
        };
        component_cfgs.push(comp2);

        let comp3: ComponentConfig = ComponentConfig {
            name: "m2".to_string(),
            model: "rdk:builtin:fake_with_dep".to_string(),
            r#type: "motor".to_string(),
            namespace: "rdk".to_string(),
            frame: None,
            depends_on: Vec::new(),
            service_configs: Vec::new(),
            api: "blah".to_string(),
            attributes: Some(Struct {
                fields: HashMap::from([(
                    "encoder".to_string(),
                    google::protobuf::Value {
                        kind: Some(google::protobuf::value::Kind::StringValue(
                            "enc2".to_string(),
                        )),
                    },
                )]),
            }),
            ..Default::default()
        };
        component_cfgs.push(comp3);

        let comp4 = ComponentConfig {
            name: "enc2".to_string(),
            model: "rdk:builtin:fake".to_string(),
            r#type: "encoder".to_string(),
            namespace: "rdk".to_string(),
            frame: None,
            depends_on: Vec::new(),
            service_configs: Vec::new(),
            api: "blah".to_string(),
            attributes: Some(Struct {
                fields: HashMap::from([(
                    "fake_deg".to_string(),
                    google::protobuf::Value {
                        kind: Some(google::protobuf::value::Kind::NumberValue(180.0)),
                    },
                )]),
            }),
            ..Default::default()
        };
        component_cfgs.push(comp4);

        let robot_cfg = ConfigResponse {
            config: Some(RobotConfig {
                components: component_cfgs,
                ..Default::default()
            }),
        };

        let robot = LocalRobot::from_cloud_config(&robot_cfg, Box::default(), None);

        assert!(robot.is_ok());

        let robot = robot.unwrap();

        let m1 = robot.get_motor_by_name("m1".to_string());

        assert!(m1.is_none());

        let m2 = robot.get_motor_by_name("m2".to_string());

        assert!(m2.is_some());

        let enc = robot.get_encoder_by_name("enc2".to_string());

        assert!(enc.is_some());
    }
}
