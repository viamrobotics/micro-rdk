#![allow(dead_code)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(feature = "camera")]
use crate::camera::Camera;

use crate::{
    common::base::Base,
    common::board::Board,
    common::motor::Motor,
    common::sensor::Sensor,
    common::status::Status,
    proto::{
        common::{self, v1::ResourceName},
        robot,
    },
};
use log::*;

use super::{
    config::{Component, RobotConfigStatic},
    registry::COMPONENT_REGISTRY,
};

pub enum ResourceType {
    Motor(Arc<Mutex<dyn Motor>>),
    Board(Arc<Mutex<dyn Board>>),
    Base(Arc<Mutex<dyn Base>>),
    Sensor(Arc<Mutex<dyn Sensor>>),
    #[cfg(feature = "camera")]
    Camera(Arc<Mutex<dyn Camera>>),
}
pub type Resource = ResourceType;
pub type ResourceMap = HashMap<ResourceName, Resource>;

pub struct LocalRobot {
    resources: ResourceMap,
}

impl LocalRobot {
    pub fn new(res: ResourceMap) -> Self {
        LocalRobot { resources: res }
    }
    pub fn new_from_static(cfg: &RobotConfigStatic) -> anyhow::Result<Self> {
        let mut robot = LocalRobot {
            resources: ResourceMap::new(),
        };
        log::info!("Building robot with {:?}", &cfg);
        if let Some(components) = cfg.components.as_ref() {
            let r = components.iter().find(|x| x.get_type() == "board");
            let b = match r {
                Some(r) => {
                    let ctor = COMPONENT_REGISTRY.get_board_constructor(r.get_model())?;
                    let b = ctor(r);
                    if let Ok(b) = b {
                        Some(b)
                    } else {
                        None
                    }
                }
                None => None,
            };
            for x in components.iter() {
                match x.get_type() {
                    "motor" => {
                        let ctor = COMPONENT_REGISTRY.get_motor_constructor(x.get_model())?;
                        let m = ctor(x, b.clone())?;
                        robot
                            .resources
                            .insert(x.get_resource_name(), ResourceType::Motor(m));
                    }
                    "board" => {
                        if let Some(b) = b.as_ref() {
                            robot
                                .resources
                                .insert(x.get_resource_name(), ResourceType::Board(b.clone()));
                        }
                    }
                    &_ => {
                        log::error!("component type {} is not supported yet", x.get_type());
                        continue;
                    }
                }
            }
        }
        Ok(robot)
    }
    pub fn get_status(
        &self,
        mut msg: robot::v1::GetStatusRequest,
    ) -> anyhow::Result<Vec<robot::v1::Status>> {
        if msg.resource_names.is_empty() {
            let mut vec = Vec::with_capacity(self.resources.len());
            for (name, val) in self.resources.iter() {
                match val {
                    ResourceType::Motor(m) => {
                        let status = m.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            status,
                        });
                    }
                    ResourceType::Board(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            status,
                        });
                    }
                    ResourceType::Base(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            status,
                        });
                    }
                    ResourceType::Sensor(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
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
            match self.resources.get(&name) {
                Some(val) => {
                    match val {
                        ResourceType::Motor(m) => {
                            let status = m.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                status,
                            });
                        }
                        ResourceType::Board(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                status,
                            });
                        }
                        ResourceType::Base(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                status,
                            });
                        }
                        ResourceType::Sensor(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
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
}

#[cfg(test)]
mod tests {
    use crate::common::config::{Kind, RobotConfigStatic, StaticComponentConfig};
    use crate::common::motor::Motor;
    use crate::common::robot::LocalRobot;
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
                        phf::phf_map! {"pins" => Kind::ListValueStatic(&[Kind::StringValueStatic("11"),Kind::StringValueStatic("12"),Kind::StringValueStatic("13")])},
                    ),
                },
                StaticComponentConfig {
                    name: "motor",
                    namespace: "rdk",
                    r#type: "motor",
                    model: "fake",
                    attributes: Some(
                        phf::phf_map! {"pins" => Kind::StructValueStatic(phf::phf_map!{"pwm" => Kind::StringValueStatic("12"),"a" => Kind::StringValueStatic("29"),"b" => Kind::StringValueStatic("5")}),"board" => Kind::StringValueStatic("board"),"fake_position" => Kind::StringValueStatic("1205")},
                    ),
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
    }
}
