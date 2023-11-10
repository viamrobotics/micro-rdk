#![allow(dead_code)]

use chrono::{DateTime, FixedOffset};
use std::{
    collections::{HashMap, HashSet},
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
        app::v1::{ComponentConfig, ConfigResponse},
        common::{self, v1::ResourceName},
        robot,
    },
};
use log::*;

use super::{
    base::BaseType,
    board::BoardType,
    config::{Component, ConfigType, DynamicComponentConfig, RobotConfigStatic},
    encoder::EncoderType,
    motor::MotorType,
    movement_sensor::MovementSensorType,
    registry::{get_board_from_dependencies, ComponentRegistry, Dependency, ResourceKey},
    sensor::SensorType,
    servo::{Servo, ServoType},
};

static NAMESPACE_PREFIX: &str = "rdk:builtin:";

#[derive(Clone)]
pub enum ResourceType {
    Motor(MotorType),
    Board(BoardType),
    Base(BaseType),
    Sensor(SensorType),
    MovementSensor(MovementSensorType),
    Encoder(EncoderType),
    Servo(ServoType),
    #[cfg(feature = "camera")]
    Camera(CameraType),
}
pub type Resource = ResourceType;
pub type ResourceMap = HashMap<ResourceName, Resource>;

pub struct LocalRobot {
    resources: ResourceMap,
    build_time: Option<DateTime<FixedOffset>>,
}

fn resource_name_from_component_cfg(cfg: &ComponentConfig) -> ResourceName {
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
fn get_model_without_namespace_prefix(full_model: &mut String) -> anyhow::Result<String> {
    if !full_model.starts_with(NAMESPACE_PREFIX) {
        anyhow::bail!(
            "model name must be prefixed with 'rdk:builtin:', model name is {:?}",
            full_model
        );
    }
    let model = full_model.split_off(NAMESPACE_PREFIX.len());
    if model.is_empty() {
        anyhow::bail!("cannot use empty model name for configuring resource");
    }
    Ok(model)
}

impl LocalRobot {
    pub fn new(res: ResourceMap) -> Self {
        LocalRobot {
            resources: res,
            // We do not know build time for non-cloud micro RDK robots.
            build_time: None,
        }
    }
    pub fn new_from_static(cfg: &RobotConfigStatic) -> anyhow::Result<Self> {
        let mut robot = LocalRobot {
            resources: ResourceMap::new(),
            // We do not know build time for non-cloud micro RDK robots.
            build_time: None,
        };
        let mut registry = ComponentRegistry::default();
        if let Some(components) = cfg.components.as_ref() {
            let mut b_r_key: Option<ResourceKey> = None;
            let r = components.iter().find(|x| x.get_type() == "board");
            let b = match r {
                Some(r) => {
                    b_r_key = Some(ResourceKey::new(
                        crate::common::board::COMPONENT_NAME,
                        r.name.to_string(),
                    )?);
                    let ctor = registry.get_board_constructor(r.get_model().to_string())?;
                    let b = ctor(ConfigType::Static(r));
                    if let Ok(b) = b {
                        Some(b)
                    } else {
                        log::info!("failed to build the board with {:?}", b.err().unwrap());
                        None
                    }
                }
                None => None,
            };
            for x in components.iter() {
                let mut deps = Vec::new();
                if let Some(b) = b.as_ref() {
                    if let Some(b_r_key) = b_r_key.as_ref() {
                        let dep = Dependency(b_r_key.clone(), Resource::Board(b.clone()));
                        deps.push(dep);
                    }
                }
                match robot.insert_resource(
                    x.get_model().to_string(),
                    x.get_resource_name(),
                    ConfigType::Static(x),
                    deps,
                    &mut registry,
                ) {
                    Ok(()) => {
                        continue;
                    }
                    Err(err) => {
                        log::error!("{:?}", err);
                        continue;
                    }
                };
            }
        };
        Ok(robot)
    }

    // Creates a robot from the response of a gRPC call to acquire the robot configuration. The individual
    // component configs within the response are consumed and the corresponding components are generated
    // and added to the created robot.
    pub fn new_from_config_response(
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

        let components = &config_resp.config.as_ref().unwrap().components;

        let board_config = components.iter().find(|x| x.r#type == "board");
        // Initialize the board component first
        let mut board_resource_key: Option<ResourceKey> = None;
        let board = match board_config {
            Some(config) => {
                let model = get_model_without_namespace_prefix(&mut config.model.to_string())?;
                board_resource_key = Some(ResourceKey::new(
                    crate::common::board::COMPONENT_NAME,
                    config.name.to_string(),
                )?);
                let cfg: DynamicComponentConfig = match config.try_into() {
                    Ok(cfg) => cfg,
                    Err(err) => {
                        anyhow::bail!("could not configure board: {:?}", err);
                    }
                };
                let constructor = registry.get_board_constructor(model)?;
                let board = constructor(ConfigType::Dynamic(cfg));
                if let Ok(board) = board {
                    Some(board)
                } else {
                    log::info!("failed to build the board with {:?}", board.err().unwrap());
                    None
                }
            }
            None => None,
        };

        let mut components_queue: Vec<&ComponentConfig> = components.iter().collect();

        robot.insert_resources(&mut components_queue, board, board_resource_key, registry)?;

        Ok(robot)
    }

    // Inserts components in order of dependency. If a component's dependencies are not satisfied it is
    // temporarily skipped and sent to the end of the queue. This process repeats until all the components
    // are added (or a max number of iterations are reached, indicating a configuration error). We have not
    // selected the most time-efficient algorithm for solving this problem in order to minimize memory usage
    fn insert_resources(
        &mut self,
        components: &mut Vec<&ComponentConfig>,
        board: Option<BoardType>,
        board_resource_key: Option<ResourceKey>,
        registry: Box<ComponentRegistry>,
    ) -> anyhow::Result<()> {
        let mut registry = registry;
        let mut inserted_resources = HashSet::<ResourceKey>::new();
        inserted_resources.try_reserve(components.len())?;

        let mut num_iterations = 0;
        let max_iterations = components.len() * 2;
        while !components.is_empty() && num_iterations < max_iterations {
            let comp_cfg = components.remove(0);
            let resource_name = resource_name_from_component_cfg(comp_cfg);
            let model = get_model_without_namespace_prefix(&mut comp_cfg.model.to_string())?;
            let dyn_config: DynamicComponentConfig = match comp_cfg.try_into() {
                Ok(cfg) => cfg,
                Err(err) => {
                    log::error!(
                        "could not configure component {:?}: {:?}",
                        comp_cfg.name,
                        err
                    );
                    continue;
                }
            };
            let c_type_static = match dyn_config.get_type() {
                "motor" => crate::common::motor::COMPONENT_NAME,
                "board" => crate::common::board::COMPONENT_NAME,
                "encoder" => crate::common::encoder::COMPONENT_NAME,
                "movement_sensor" => crate::common::movement_sensor::COMPONENT_NAME,
                "sensor" => crate::common::sensor::COMPONENT_NAME,
                "base" => crate::common::base::COMPONENT_NAME,
                "servo" => crate::common::servo::COMPONENT_NAME,
                &_ => {
                    anyhow::bail!(
                        "component type {} is not supported yet",
                        dyn_config.get_type()
                    );
                }
            };
            let resource_key = ResourceKey::new(c_type_static, resource_name.name.to_string())?;

            if !inserted_resources.contains(&resource_key) {
                match registry.get_dependency_function(c_type_static, model.to_string()) {
                    Ok(dependency_getter) => {
                        let dependency_resource_keys =
                            dependency_getter(ConfigType::Dynamic(dyn_config));
                        if dependency_resource_keys
                            .iter()
                            .all(|x| inserted_resources.contains(x))
                        {
                            let dependencies_result: Result<Vec<Dependency>, anyhow::Error> =
                                dependency_resource_keys
                                    .iter()
                                    .map(|dep_key| {
                                        let dep_r_name = ResourceName {
                                            namespace: resource_name.namespace.to_string(),
                                            r#type: resource_name.r#type.to_string(),
                                            subtype: dep_key.0.to_string(),
                                            name: dep_key.1.to_string(),
                                        };
                                        let res = match self.resources.get(&dep_r_name) {
                                            Some(r) => r.clone(),
                                            None => anyhow::bail!("dependency not created yet"),
                                        };
                                        let dep_key_copy =
                                            ResourceKey(dep_key.0, dep_key.1.to_string());
                                        Ok(Dependency(dep_key_copy, res))
                                    })
                                    .collect();
                            let mut dependencies = dependencies_result?;
                            if let Some(b) = board.as_ref() {
                                if let Some(b_r_key) = board_resource_key.as_ref() {
                                    let dep =
                                        Dependency(b_r_key.clone(), Resource::Board(b.clone()));
                                    dependencies.push(dep);
                                }
                            }
                            match self.insert_resource(
                                model.to_string(),
                                resource_name,
                                ConfigType::Dynamic(comp_cfg.try_into()?),
                                dependencies,
                                &mut registry,
                            ) {
                                Ok(()) => {}
                                Err(err) => {
                                    log::error!("{:?}", err);
                                    continue;
                                }
                            };
                            inserted_resources.insert(resource_key);
                        } else {
                            let model = model.to_string();
                            log::debug!("skipping {model} for now...");
                            components.push(comp_cfg)
                        }
                    }
                    Err(_) => {
                        let mut dependencies = Vec::new();
                        if let Some(b) = board.as_ref() {
                            if let Some(b_r_key) = board_resource_key.as_ref() {
                                let dep = Dependency(b_r_key.clone(), Resource::Board(b.clone()));
                                dependencies.push(dep);
                            }
                        }
                        match self.insert_resource(
                            model.to_string(),
                            resource_name,
                            ConfigType::Dynamic(comp_cfg.try_into()?),
                            dependencies,
                            &mut registry,
                        ) {
                            Ok(()) => {}
                            Err(err) => {
                                log::error!("{:?}", err);
                                continue;
                            }
                        };
                        inserted_resources.insert(resource_key);
                    }
                };
            }
            num_iterations += 1;
        }
        if !components.is_empty() {
            log::error!(
                "Some components not created because their dependencies were never met.
                Check your config for missing components or circular dependencies.
                Uncreated component configs: {:?}",
                components
            );
        }
        Ok(())
    }

    fn insert_resource(
        &mut self,
        model: String,
        r_name: ResourceName,
        cfg: ConfigType,
        deps: Vec<Dependency>,
        registry: &mut ComponentRegistry,
    ) -> anyhow::Result<()> {
        let r_type = cfg.get_type();
        let res = match r_type {
            "motor" => {
                let ctor = registry.get_motor_constructor(model)?;
                ResourceType::Motor(ctor(cfg, deps)?)
            }
            "board" => {
                let board = get_board_from_dependencies(deps);
                ResourceType::Board(match board {
                    Some(b) => b.clone(),
                    None => return Ok(()),
                })
            }
            "sensor" => {
                let ctor = registry.get_sensor_constructor(model)?;
                ResourceType::Sensor(ctor(cfg, deps)?)
            }
            "movement_sensor" => {
                let ctor = registry.get_movement_sensor_constructor(model)?;
                ResourceType::MovementSensor(ctor(cfg, deps)?)
            }
            "encoder" => {
                let ctor = registry.get_encoder_constructor(model)?;
                ResourceType::Encoder(ctor(cfg, deps)?)
            }
            "base" => {
                let ctor = registry.get_base_constructor(model)?;
                ResourceType::Base(ctor(cfg, deps)?)
            }
            "servo" => {
                let ctor = registry.get_servo_constructor(model)?;
                ResourceType::Servo(ctor(cfg, deps)?)
            }
            &_ => {
                anyhow::bail!("component type {} is not supported yet", r_type);
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
                    ResourceType::Servo(b) => {
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
                        ResourceType::Servo(b) => {
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

    pub fn stop_all(&mut self) -> anyhow::Result<()> {
        let mut stop_errors: Vec<anyhow::Error> = vec![];
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
            anyhow::bail!(
                "Could not stop all robot actuators, following errors encountered: {:?}",
                stop_errors
            )
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::board::Board;
    use crate::common::config::{Kind, RobotConfigStatic, StaticComponentConfig};
    use crate::common::encoder::{Encoder, EncoderPositionType};
    use crate::common::i2c::I2CHandle;
    use crate::common::motor::Motor;
    use crate::common::movement_sensor::MovementSensor;
    use crate::common::registry::ComponentRegistry;
    use crate::common::robot::{LocalRobot, ResourceMap};
    use crate::common::sensor::Sensor;
    use crate::google;
    use crate::google::protobuf::Struct;
    use crate::proto::app::v1::ComponentConfig;

    #[test_log::test]
    fn test_robot_from_static() {
        #[allow(clippy::redundant_static_lifetimes, dead_code)]
        const STATIC_ROBOT_CONFIG: Option<RobotConfigStatic> = Some(RobotConfigStatic {
            components: Some(&[
                StaticComponentConfig {
                    name: "board",
                    namespace: "rdk",
                    r#type: "board",
                    model: "fake",
                    attributes: Some(
                        phf::phf_map! {"pins" => Kind::ListValueStatic(&[Kind::StringValueStatic("11"),Kind::StringValueStatic("12"),Kind::StringValueStatic("13")]),
                        "analogs" => Kind::StructValueStatic(phf::phf_map!{"1" => Kind::StringValueStatic("11.12")}),
                        "i2cs" => Kind::ListValueStatic(&[
                            Kind::StructValueStatic(phf::phf_map!{"name" => Kind::StringValueStatic("i2c0")}),
                            Kind::StructValueStatic(phf::phf_map!{
                                "name" => Kind::StringValueStatic("i2c1"),
                                "value_1" => Kind::StringValueStatic("5"),
                                "value_2" => Kind::StringValueStatic("4")
                            })
                        ])},
                    ),
                },
                StaticComponentConfig {
                    name: "motor",
                    namespace: "rdk",
                    r#type: "motor",
                    model: "fake",
                    attributes: Some(phf::phf_map! {
                    "pins" => Kind::StructValueStatic(phf::phf_map!{
                        "pwm" => Kind::StringValueStatic("12"),
                        "a" => Kind::StringValueStatic("29"),
                        "b" => Kind::StringValueStatic("5")}
                    ),
                    "board" => Kind::StringValueStatic("board"),
                    "max_rpm" => Kind::StringValueStatic("100"),
                    "fake_position" => Kind::StringValueStatic("1205")}),
                },
                StaticComponentConfig {
                    name: "sensor",
                    namespace: "rdk",
                    r#type: "sensor",
                    model: "fake",
                    attributes: Some(
                        phf::phf_map! {"fake_value" => Kind::StringValueStatic("11.12")},
                    ),
                },
                StaticComponentConfig {
                    name: "m_sensor",
                    namespace: "rdk",
                    r#type: "movement_sensor",
                    model: "fake",
                    attributes: Some(phf::phf_map! {
                        "fake_lat" => Kind::StringValueStatic("68.86"),
                        "fake_lon" => Kind::StringValueStatic("-85.44"),
                        "fake_alt" => Kind::StringValueStatic("3000.1"),
                        "lin_acc_x" => Kind::StringValueStatic("200.2"),
                        "lin_acc_y" => Kind::StringValueStatic("-100.3"),
                        "lin_acc_z" => Kind::StringValueStatic("100.4"),
                    }),
                },
                StaticComponentConfig {
                    name: "enc1",
                    namespace: "rdk",
                    r#type: "encoder",
                    model: "fake",
                    attributes: Some(phf::phf_map! {
                        "fake_deg" => Kind::StringValueStatic("45.0"),
                        "ticks_per_rotation" => Kind::StringValueStatic("2"),
                    }),
                },
                StaticComponentConfig {
                    name: "enc2",
                    namespace: "rdk",
                    r#type: "encoder",
                    model: "fake_incremental",
                    attributes: Some(phf::phf_map! {
                        "fake_ticks" => Kind::StringValueStatic("3.0"),
                    }),
                },
            ]),
        });
        let robot = LocalRobot::new_from_static(&STATIC_ROBOT_CONFIG.unwrap());
        assert!(robot.is_ok());
        let robot = robot.unwrap();

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
            .borrow_mut()
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
    fn test_insert_resources() {
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
        };
        component_cfgs.push(&comp);

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
        };
        component_cfgs.push(&comp2);

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
        };
        component_cfgs.push(&comp3);

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
        };
        component_cfgs.push(&comp4);

        let mut robot = LocalRobot {
            resources: ResourceMap::new(),
            build_time: None,
        };

        assert!(robot
            .insert_resources(
                &mut component_cfgs,
                None,
                None,
                Box::<ComponentRegistry>::default()
            )
            .is_ok());

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
    fn test_insert_resources_unmet_dependency() {
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
        };
        component_cfgs.push(&comp2);

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
        };
        component_cfgs.push(&comp3);

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
        };
        component_cfgs.push(&comp4);

        let mut robot = LocalRobot {
            resources: ResourceMap::new(),
            build_time: None,
        };

        assert!(robot
            .insert_resources(
                &mut component_cfgs,
                None,
                None,
                Box::<ComponentRegistry>::default(),
            )
            .is_ok());

        let m1 = robot.get_motor_by_name("m1".to_string());

        assert!(m1.is_none());

        let m2 = robot.get_motor_by_name("m2".to_string());

        assert!(m2.is_some());

        let enc = robot.get_encoder_by_name("enc2".to_string());

        assert!(enc.is_some());
    }
}
