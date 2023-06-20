#![allow(dead_code)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(feature = "camera")]
use crate::camera::{Camera, CameraType};

use crate::{
    common::base::Base,
    common::board::Board,
    common::encoder::Encoder,
    common::motor::Motor,
    common::movement_sensor::MovementSensor,
    common::sensor::Sensor,
    common::status::Status,
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
    registry::COMPONENT_REGISTRY,
    sensor::SensorType,
};

pub enum ResourceType {
    Motor(MotorType),
    Board(BoardType),
    Base(BaseType),
    Sensor(SensorType),
    MovementSensor(MovementSensorType),
    Encoder(EncoderType),
    #[cfg(feature = "camera")]
    Camera(CameraType),
}
pub type Resource = ResourceType;
pub type ResourceMap = HashMap<ResourceName, Resource>;

pub struct LocalRobot {
    resources: ResourceMap,
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
// auto-prefixed with "rdk:builtin". A more ideal and robust method of namespacing is preferred.
fn get_model_without_micro_rdk_prefix(full_model: &mut String) -> anyhow::Result<String> {
    if !full_model.starts_with("micro-rdk:builtin") {
        anyhow::bail!(
            "model name must be prefixed with 'micro-rdk:builtin', model name is {:?}",
            full_model
        );
    }
    let model = full_model.split_off(18);
    if model.is_empty() {
        anyhow::bail!("cannot use empty model name for configuring resource");
    }
    Ok(model)
}

impl LocalRobot {
    pub fn new(res: ResourceMap) -> Self {
        LocalRobot { resources: res }
    }
    pub fn new_from_static(cfg: &RobotConfigStatic) -> anyhow::Result<Self> {
        let mut robot = LocalRobot {
            resources: ResourceMap::new(),
        };
        if let Some(components) = cfg.components.as_ref() {
            let r = components.iter().find(|x| x.get_type() == "board");
            let b = match r {
                Some(r) => {
                    let ctor =
                        COMPONENT_REGISTRY.get_board_constructor(r.get_model().to_string())?;
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
                match robot.insert_resource(
                    x.get_model().to_string(),
                    x.get_resource_name(),
                    ConfigType::Static(x),
                    b.clone(),
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
        }
        Ok(robot)
    }

    // Creates a robot from the response of a gRPC call to acquire the robot configuration. The individual
    // component configs within the response are consumed and the corresponding components are generated
    // and added to the created robot.
    pub fn new_from_config_response(config_resp: &ConfigResponse) -> anyhow::Result<Self> {
        let mut robot = LocalRobot {
            resources: ResourceMap::new(),
        };

        let components = &(config_resp.config.as_ref().unwrap().components);

        let r = components.iter().find(|x| x.r#type == "board");
        // Initialize the board component first
        let b = match r {
            Some(r) => {
                let model = get_model_without_micro_rdk_prefix(&mut r.model.to_string())?;

                let cfg: DynamicComponentConfig = match r.try_into() {
                    Ok(cfg) => cfg,
                    Err(err) => {
                        anyhow::bail!("could not configure board: {:?}", err);
                    }
                };
                let ctor = COMPONENT_REGISTRY.get_board_constructor(model)?;
                let b = ctor(ConfigType::Dynamic(cfg));
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
            let model = get_model_without_micro_rdk_prefix(&mut x.model.to_string())?;
            let r_name = resource_name_from_component_cfg(x);
            let dyn_config: DynamicComponentConfig = match x.try_into() {
                Ok(cfg) => cfg,
                Err(err) => {
                    log::error!("could not configure component {:?}: {:?}", x.name, err);
                    continue;
                }
            };
            match robot.insert_resource(model, r_name, ConfigType::Dynamic(dyn_config), b.clone()) {
                Ok(()) => {
                    continue;
                }
                Err(err) => {
                    log::error!("{:?}", err);
                    continue;
                }
            };
        }

        Ok(robot)
    }

    fn insert_resource(
        &mut self,
        model: String,
        r_name: ResourceName,
        cfg: ConfigType,
        board: Option<BoardType>,
    ) -> anyhow::Result<()> {
        let r_type = cfg.get_type();
        let res = match r_type {
            "motor" => {
                let ctor = COMPONENT_REGISTRY.get_motor_constructor(model)?;
                ResourceType::Motor(ctor(cfg, board)?)
            }
            "board" => {
                if let Some(board) = board.as_ref() {
                    ResourceType::Board(board.clone())
                } else {
                    return Ok(());
                }
            }
            "sensor" => {
                let ctor = COMPONENT_REGISTRY.get_sensor_constructor(model)?;
                ResourceType::Sensor(ctor(cfg, board)?)
            }
            "movement_sensor" => {
                let ctor = COMPONENT_REGISTRY.get_movement_sensor_constructor(model)?;
                ResourceType::MovementSensor(ctor(cfg, board)?)
            }
            "encoder" => {
                let ctor = COMPONENT_REGISTRY.get_encoder_constructor(model)?;
                ResourceType::Encoder(ctor(cfg, board)?)
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
        if msg.resource_names.is_empty() {
            let mut vec = Vec::with_capacity(self.resources.len());
            for (name, val) in self.resources.iter_mut() {
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
                    ResourceType::MovementSensor(b) => {
                        let status = b.get_status()?;
                        vec.push(robot::v1::Status {
                            name: Some(name.clone()),
                            status,
                        });
                    }
                    ResourceType::Encoder(b) => {
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
            match self.resources.get_mut(&name) {
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
                        ResourceType::MovementSensor(b) => {
                            let status = b.get_status()?;
                            vec.push(robot::v1::Status {
                                name: Some(name),
                                status,
                            });
                        }
                        ResourceType::Encoder(b) => {
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
}

#[cfg(test)]
mod tests {
    use crate::common::board::Board;
    use crate::common::config::{Kind, RobotConfigStatic, StaticComponentConfig};
    use crate::common::encoder::{Encoder, EncoderPositionType};
    use crate::common::i2c::I2CHandle;
    use crate::common::motor::Motor;
    use crate::common::movement_sensor::MovementSensor;
    use crate::common::robot::LocalRobot;
    use crate::common::sensor::Sensor;

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
            Some(prost_types::value::Kind::NumberValue(a)) => Some(a),
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
}
