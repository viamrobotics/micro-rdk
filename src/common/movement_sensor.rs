#![allow(dead_code)]
use super::board::BoardType;
use super::config::Component;
use super::config::ConfigType;
use super::math_utils::Vector3;
use super::registry::ComponentRegistry;
use super::status::Status;
use crate::proto::common::v1::GeoPoint;
use crate::proto::component::movement_sensor;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_movement_sensor("fake", &FakeMovementSensor::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
}

// A local struct representation of the supported methods indicated by the
// GetProperties method of the Movement Sensor API. TODO: add a boolean for
// orientation when it is supportable.
pub struct MovementSensorSupportedMethods {
    pub position_supported: bool,
    pub linear_velocity_supported: bool,
    pub angular_velocity_supported: bool,
    pub linear_acceleration_supported: bool,
    pub compass_heading_supported: bool,
}

impl From<MovementSensorSupportedMethods> for movement_sensor::v1::GetPropertiesResponse {
    fn from(props: MovementSensorSupportedMethods) -> movement_sensor::v1::GetPropertiesResponse {
        movement_sensor::v1::GetPropertiesResponse {
            position_supported: props.position_supported,
            linear_velocity_supported: props.linear_velocity_supported,
            angular_velocity_supported: props.angular_velocity_supported,
            linear_acceleration_supported: props.linear_acceleration_supported,
            compass_heading_supported: props.compass_heading_supported,
            orientation_supported: false,
        }
    }
}

// A struct representing geographic coordinates (latitude-longitude-altitude)
#[derive(Clone, Copy, Debug, Default)]
pub struct GeoPosition {
    pub lat: f64,
    pub lon: f64,
    pub alt: f32,
}

impl From<GeoPosition> for movement_sensor::v1::GetPositionResponse {
    fn from(pos: GeoPosition) -> movement_sensor::v1::GetPositionResponse {
        let pt = GeoPoint {
            latitude: pos.lat,
            longitude: pos.lon,
        };
        movement_sensor::v1::GetPositionResponse {
            coordinate: Some(pt),
            altitude_mm: pos.alt,
        }
    }
}

// A trait for implementing a movement sensor component driver. TODO: add
// get_orientation and get_accuracy if/when they become supportable.
pub trait MovementSensor: Status {
    fn get_position(&mut self) -> anyhow::Result<GeoPosition>;
    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3>;
    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3>;
    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3>;
    fn get_compass_heading(&mut self) -> anyhow::Result<f64>;
    fn get_properties(&self) -> MovementSensorSupportedMethods;
}

pub(crate) type MovementSensorType = Arc<Mutex<dyn MovementSensor>>;

pub struct FakeMovementSensor {
    pos: GeoPosition,
    linear_acc: Vector3,
}

impl Default for FakeMovementSensor {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeMovementSensor {
    pub fn new() -> Self {
        FakeMovementSensor {
            pos: GeoPosition {
                lat: 27.33,
                lon: 29.45,
                alt: 4572.2,
            },
            linear_acc: Vector3 {
                x: 5.0,
                y: 2.0,
                z: 3.0,
            },
        }
    }
    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Option<BoardType>,
    ) -> anyhow::Result<MovementSensorType> {
        match cfg {
            ConfigType::Static(cfg) => {
                let mut fake_pos: GeoPosition = Default::default();
                if let Ok(fake_lat) = cfg.get_attribute::<f64>("fake_lat") {
                    fake_pos.lat = fake_lat
                }
                if let Ok(fake_lon) = cfg.get_attribute::<f64>("fake_lon") {
                    fake_pos.lon = fake_lon
                }
                if let Ok(fake_alt) = cfg.get_attribute::<f32>("fake_alt") {
                    fake_pos.alt = fake_alt
                }

                let mut lin_acc: Vector3 = Default::default();
                if let Ok(x) = cfg.get_attribute::<f64>("lin_acc_x") {
                    lin_acc.x = x
                }
                if let Ok(y) = cfg.get_attribute::<f64>("lin_acc_y") {
                    lin_acc.y = y
                }
                if let Ok(z) = cfg.get_attribute::<f64>("lin_acc_z") {
                    lin_acc.z = z
                }

                Ok(Arc::new(Mutex::new(FakeMovementSensor {
                    pos: fake_pos,
                    linear_acc: lin_acc,
                })))
            }
        }
    }
}

impl MovementSensor for FakeMovementSensor {
    fn get_position(&mut self) -> anyhow::Result<GeoPosition> {
        Ok(self.pos)
    }

    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3> {
        Ok(self.linear_acc)
    }

    fn get_properties(&self) -> MovementSensorSupportedMethods {
        MovementSensorSupportedMethods {
            position_supported: true,
            linear_acceleration_supported: true,
            linear_velocity_supported: false,
            angular_velocity_supported: false,
            compass_heading_supported: false,
        }
    }

    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3> {
        anyhow::bail!("unimplemented: movement_sensor_get_linear_velocity")
    }

    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3> {
        anyhow::bail!("unimplemented: movement_sensor_get_angular_velocity")
    }

    fn get_compass_heading(&mut self) -> anyhow::Result<f64> {
        anyhow::bail!("unimplemented: movement_sensor_get_compass_heading")
    }
}

impl Status for FakeMovementSensor {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}

impl<A> MovementSensor for Mutex<A>
where
    A: ?Sized + MovementSensor,
{
    fn get_position(&mut self) -> anyhow::Result<GeoPosition> {
        self.get_mut().unwrap().get_position()
    }

    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3> {
        self.get_mut().unwrap().get_linear_acceleration()
    }

    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3> {
        self.get_mut().unwrap().get_linear_velocity()
    }

    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3> {
        self.get_mut().unwrap().get_angular_velocity()
    }

    fn get_compass_heading(&mut self) -> anyhow::Result<f64> {
        self.get_mut().unwrap().get_compass_heading()
    }

    fn get_properties(&self) -> MovementSensorSupportedMethods {
        self.lock().unwrap().get_properties()
    }
}

impl<A> MovementSensor for Arc<Mutex<A>>
where
    A: ?Sized + MovementSensor,
{
    fn get_position(&mut self) -> anyhow::Result<GeoPosition> {
        self.lock().unwrap().get_position()
    }

    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3> {
        self.lock().unwrap().get_linear_acceleration()
    }

    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3> {
        self.lock().unwrap().get_linear_velocity()
    }

    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3> {
        self.lock().unwrap().get_angular_velocity()
    }

    fn get_compass_heading(&mut self) -> anyhow::Result<f64> {
        self.lock().unwrap().get_compass_heading()
    }

    fn get_properties(&self) -> MovementSensorSupportedMethods {
        self.lock().unwrap().get_properties()
    }
}
