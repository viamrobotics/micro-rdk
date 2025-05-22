#![allow(dead_code)]

#[cfg(feature = "builtin-components")]
use {
    super::config::ConfigType,
    super::registry::{ComponentRegistry, Dependency},
};

use super::generic::DoCommand;
use super::math_utils::Vector3;
use super::sensor::{GenericReadingsResult, Readings, SensorError};
use crate::google;
use crate::google::protobuf::{value::Kind, Struct, Value};
use crate::proto::common::v1::GeoPoint;
use crate::proto::component::movement_sensor;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub static COMPONENT_NAME: &str = "movement_sensor";

#[cfg(feature = "builtin-components")]
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

impl From<GeoPosition> for Value {
    fn from(value: GeoPosition) -> Self {
        let mut fields = HashMap::new();
        fields.insert(
            "lat".to_string(),
            Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(value.lat)),
            },
        );
        fields.insert(
            "lon".to_string(),
            Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(value.lon)),
            },
        );
        fields.insert(
            "alt".to_string(),
            Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(value.alt as f64)),
            },
        );
        Value {
            kind: Some(google::protobuf::value::Kind::StructValue(Struct {
                fields,
            })),
        }
    }
}

impl From<GeoPosition> for movement_sensor::v1::GetPositionResponse {
    fn from(pos: GeoPosition) -> movement_sensor::v1::GetPositionResponse {
        let pt = GeoPoint {
            latitude: pos.lat,
            longitude: pos.lon,
        };
        movement_sensor::v1::GetPositionResponse {
            coordinate: Some(pt),
            altitude_m: pos.alt,
        }
    }
}

// A trait for implementing a movement sensor component driver. TODO: add
// get_orientation and get_accuracy if/when they become supportable.
pub trait MovementSensor: Readings + DoCommand {
    fn get_position(&mut self) -> Result<GeoPosition, SensorError>;
    fn get_linear_velocity(&mut self) -> Result<Vector3, SensorError>;
    fn get_angular_velocity(&mut self) -> Result<Vector3, SensorError>;
    fn get_linear_acceleration(&mut self) -> Result<Vector3, SensorError>;
    fn get_compass_heading(&mut self) -> Result<f64, SensorError>;
    fn get_properties(&self) -> MovementSensorSupportedMethods;
}

pub type MovementSensorType = Arc<Mutex<dyn MovementSensor>>;

pub fn get_movement_sensor_generic_readings(
    ms: &mut dyn MovementSensor,
) -> Result<GenericReadingsResult, SensorError> {
    let mut res = std::collections::HashMap::new();
    let supported_methods = ms.get_properties();
    if supported_methods.position_supported {
        res.insert("position".to_string(), ms.get_position()?.into());
    }
    if supported_methods.linear_velocity_supported {
        res.insert(
            "linear_velocity".to_string(),
            ms.get_linear_velocity()?.into(),
        );
    }
    if supported_methods.linear_acceleration_supported {
        res.insert(
            "linear_acceleration".to_string(),
            ms.get_linear_acceleration()?.into(),
        );
    }
    if supported_methods.angular_velocity_supported {
        res.insert(
            "angular_velocity".to_string(),
            ms.get_angular_velocity()?.into(),
        );
    }
    if supported_methods.compass_heading_supported {
        res.insert(
            "compass_heading".to_string(),
            Value {
                kind: Some(Kind::NumberValue(ms.get_compass_heading()?)),
            },
        );
    }
    Ok(res)
}

#[cfg(feature = "builtin-components")]
#[derive(DoCommand, MovementSensorReadings)]
pub struct FakeMovementSensor {
    pos: GeoPosition,
    linear_acc: Vector3,
}

#[cfg(feature = "builtin-components")]
impl Default for FakeMovementSensor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "builtin-components")]
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
        _: Vec<Dependency>,
    ) -> Result<MovementSensorType, SensorError> {
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

#[cfg(feature = "builtin-components")]
impl MovementSensor for FakeMovementSensor {
    fn get_position(&mut self) -> Result<GeoPosition, SensorError> {
        Ok(self.pos)
    }

    fn get_linear_acceleration(&mut self) -> Result<Vector3, SensorError> {
        Ok(self.linear_acc)
    }

    fn get_properties(&self) -> MovementSensorSupportedMethods {
        MovementSensorSupportedMethods {
            position_supported: true,
            linear_acceleration_supported: true,
            linear_velocity_supported: false,
            angular_velocity_supported: false,
            compass_heading_supported: true,
        }
    }

    fn get_linear_velocity(&mut self) -> Result<Vector3, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "get_linear_velocity",
        ))
    }

    fn get_angular_velocity(&mut self) -> Result<Vector3, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "get_angular_velocity",
        ))
    }

    fn get_compass_heading(&mut self) -> Result<f64, SensorError> {
        Ok(42.)
    }
}

impl<A> MovementSensor for Mutex<A>
where
    A: ?Sized + MovementSensor,
{
    fn get_position(&mut self) -> Result<GeoPosition, SensorError> {
        self.get_mut().unwrap().get_position()
    }

    fn get_linear_acceleration(&mut self) -> Result<Vector3, SensorError> {
        self.get_mut().unwrap().get_linear_acceleration()
    }

    fn get_linear_velocity(&mut self) -> Result<Vector3, SensorError> {
        self.get_mut().unwrap().get_linear_velocity()
    }

    fn get_angular_velocity(&mut self) -> Result<Vector3, SensorError> {
        self.get_mut().unwrap().get_angular_velocity()
    }

    fn get_compass_heading(&mut self) -> Result<f64, SensorError> {
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
    fn get_position(&mut self) -> Result<GeoPosition, SensorError> {
        self.lock().unwrap().get_position()
    }

    fn get_linear_acceleration(&mut self) -> Result<Vector3, SensorError> {
        self.lock().unwrap().get_linear_acceleration()
    }

    fn get_linear_velocity(&mut self) -> Result<Vector3, SensorError> {
        self.lock().unwrap().get_linear_velocity()
    }

    fn get_angular_velocity(&mut self) -> Result<Vector3, SensorError> {
        self.lock().unwrap().get_angular_velocity()
    }

    fn get_compass_heading(&mut self) -> Result<f64, SensorError> {
        self.lock().unwrap().get_compass_heading()
    }

    fn get_properties(&self) -> MovementSensorSupportedMethods {
        self.lock().unwrap().get_properties()
    }
}
