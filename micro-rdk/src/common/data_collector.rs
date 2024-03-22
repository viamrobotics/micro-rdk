use std::fmt::Display;

use crate::proto::app::data_sync::v1::SensorData;

use super::{
    config::{AttributeError, Kind},
    movement_sensor::MovementSensor,
    robot::ResourceType,
    sensor::{get_sensor_readings_data, SensorError},
};

use thiserror::Error;

/// A DataCollectorConfig instance is a representation of an element
/// of the list of "capture_methods" in the "attributes" section of a
/// component's configuration JSON object as stored in app. Each element
/// of "capture_methods" is meant to produce an instance of `DataCollector`
/// as defined below
#[derive(Debug, Clone)]
pub struct DataCollectorConfig {
    pub method: CollectionMethod,
    pub capture_frequency_hz: f32,
}

impl TryFrom<&Kind> for DataCollectorConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if !value.contains_key("method")? {
            return Err(AttributeError::KeyNotFound("method".to_string()));
        }
        if !value.contains_key("capture_frequency_hz")? {
            return Err(AttributeError::KeyNotFound(
                "capture_frequency_hz".to_string(),
            ));
        }
        let method_str: String = value
            .get("method")?
            .ok_or(AttributeError::KeyNotFound("method".to_string()))?
            .try_into()?;
        let capture_frequency_hz = value
            .get("capture_frequency_hz")?
            .ok_or(AttributeError::KeyNotFound(
                "capture_frequency_hz".to_string(),
            ))?
            .try_into()?;
        // TODO: Collectors that take arguments (ex. Board Analogs)
        // let parameters: &Kind = if let Ok(Some(params)) = value.get("additional_params") {
        //     params
        // } else {
        //     &Kind::NullValue(0)
        // };
        let method = match method_str.as_str() {
            "Readings" => CollectionMethod::Readings,
            "AngularVelocity" => CollectionMethod::AngularVelocity,
            "LinearAcceleration" => CollectionMethod::LinearAcceleration,
            "LinearVelocity" => CollectionMethod::LinearVelocity,
            // TODO: Power Sensor and Board collectors
            // "Voltage" => CollectionMethod::Voltage,
            // "Current" => CollectionMethod::Current,
            // "Analogs" => {
            //     let analog_reader_name: String = parameters.get("reader_name")?.ok_or(AttributeError::KeyNotFound("reader_name".to_string()))?.try_into()?;
            //     CollectionMethod::Analogs(analog_reader_name)
            // },
            _ => {
                return Err(AttributeError::ConversionImpossibleError);
            }
        };
        Ok(DataCollectorConfig {
            method,
            capture_frequency_hz,
        })
    }
}

/// A CollectionMethod is an enum whose values are associated with
/// a method on one or more component traits
#[derive(Debug, Clone)]
pub enum CollectionMethod {
    Readings,
    // MovementSensor methods
    AngularVelocity,
    LinearAcceleration,
    LinearVelocity,
    // TODO: PowerSensor methods
    // Voltage,
    // Current,
    // TODO: Board
    // Analogs(String)
}

impl CollectionMethod {
    fn method_str(&self) -> String {
        match self {
            Self::Readings => "readings".to_string(),
            Self::AngularVelocity => "angularvelocity".to_string(),
            Self::LinearAcceleration => "linearacceleration".to_string(),
            Self::LinearVelocity => "linearvelocity".to_string(),
            // TODO: Power Sensor and Board collectors
            // Self::Voltage => "voltage".to_string(),
            // Self::Current => "current".to_string(),
            // Self::Analogs(_) => "analogs".to_string()
        }
    }
}

impl Display for CollectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.method_str(), f)
    }
}

#[derive(Error, Debug)]
pub enum DataCollectionError {
    #[error("method {0} unsupported for {1}")]
    UnsupportedMethod(CollectionMethod, String),
    #[error("no collection methods supported for component")]
    NoSupportedMethods,
    #[error(transparent)]
    SensorCollectionError(#[from] SensorError),
    // TODO: remove when error enums for all components are created (RSDK-6909)
    #[error(transparent)]
    MiscCollectionError(#[from] anyhow::Error),
}

/// A DataCollector represents an association between a data collection method and
/// a ResourceType (i.e. SensorType & Readings, BoardType & Analogs) and the frequency at
/// which the results of the method should be stored. It is capable
pub struct DataCollector {
    name: String,
    component_type: String,
    resource: ResourceType,
    method: CollectionMethod,
    time_interval_ms: u64,
}

fn resource_method_pair_is_valid(resource: &ResourceType, method: &CollectionMethod) -> bool {
    match resource {
        ResourceType::Sensor(_) => matches!(method, CollectionMethod::Readings),
        ResourceType::MovementSensor(_) => matches!(
            method,
            CollectionMethod::Readings
                | CollectionMethod::AngularVelocity
                | CollectionMethod::LinearAcceleration
                | CollectionMethod::LinearVelocity
        ),
        // ResourceType::PowerSensor(_) => matches!(method, CollectionMethod::Readings | CollectionMethod::Voltage | CollectionMethod::Current),
        // ResourceType::Board(_) => matches!(method, CollectionMethod::Analogs(_)),
        _ => false,
    }
}

impl DataCollector {
    pub fn new(
        name: String,
        resource: ResourceType,
        method: CollectionMethod,
        capture_frequency_hz: f32,
    ) -> Result<Self, DataCollectionError> {
        let time_interval_ms = ((1.0 / capture_frequency_hz) * 1000.0) as u64;
        let component_type = resource.component_type();
        if !resource_method_pair_is_valid(&resource, &method) {
            return Err(DataCollectionError::UnsupportedMethod(
                method,
                component_type,
            ));
        }
        Ok(DataCollector {
            name,
            component_type,
            resource,
            method,
            time_interval_ms,
        })
    }

    pub fn from_config(
        name: String,
        resource: ResourceType,
        conf: DataCollectorConfig,
    ) -> Result<Self, DataCollectionError> {
        Self::new(name, resource, conf.method, conf.capture_frequency_hz)
    }

    pub fn name(&self) -> String {
        self.name.to_string()
    }

    pub fn component_type(&self) -> String {
        self.component_type.to_string()
    }

    pub fn method_str(&self) -> String {
        self.method.method_str()
    }

    pub fn time_interval(&self) -> u64 {
        self.time_interval_ms
    }

    /// calls the method associated with the collector and returns the resulting data
    pub(crate) fn call_method(&mut self) -> Result<SensorData, DataCollectionError> {
        Ok(match &mut self.resource {
            ResourceType::Sensor(ref mut res) => match self.method {
                CollectionMethod::Readings => get_sensor_readings_data(res)?,
                _ => {
                    return Err(DataCollectionError::UnsupportedMethod(
                        self.method.clone(),
                        "sensor".to_string(),
                    ))
                }
            },
            ResourceType::MovementSensor(ref mut res) => match self.method {
                CollectionMethod::Readings => get_sensor_readings_data(res)?,
                CollectionMethod::AngularVelocity => res
                    .get_angular_velocity()?
                    .to_sensor_data("angular_velocity"),
                CollectionMethod::LinearAcceleration => res
                    .get_linear_acceleration()?
                    .to_sensor_data("linear_acceleration"),
                CollectionMethod::LinearVelocity => {
                    res.get_linear_velocity()?.to_sensor_data("linear_velocity")
                }
                _ => {
                    return Err(DataCollectionError::UnsupportedMethod(
                        self.method.clone(),
                        "movement_sensor".to_string(),
                    ))
                }
            },
            // TODO: PowerSensor + Board collectors
            // ResourceType::PowerSensor(ref mut res) => match self.method {
            //     CollectionMethod::Voltage => res.get_voltage()?.into(),
            //     CollectionMethod::Current => res.get_current()?.into(),
            //     _ => return Err(DataCollectionError::UnsupportedMethod(self.method, "power_sensor")),
            // },
            // ResourceType::Board(ref mut res) => match &self.method {
            //     CollectionMethod::Analogs(name) => {
            //         get_analog_readings_data(res, name.to_string())?
            //     },
            //     _ => unreachable!()
            // }
            _ => return Err(DataCollectionError::NoSupportedMethods),
        })
    }

    // TODO: will call collect_data and store on a cache yet to be implemented
    // pub fn collect_data(&mut self) -> Result<(), DataCollectionError> {

    // }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use super::{CollectionMethod, DataCollectionError, DataCollector, DataCollectorConfig};
    use crate::common::config::{AttributeError, Kind};
    use crate::common::robot::ResourceType;
    use crate::common::sensor::FakeSensor;
    use crate::google;
    use crate::proto::app::data_sync::v1::sensor_data::Data;

    #[test_log::test]
    fn test_collector_config() -> Result<(), AttributeError> {
        let kind_map = HashMap::from([
            (
                "method".to_string(),
                Kind::StringValue("Readings".to_string()),
            ),
            ("capture_frequency_hz".to_string(), Kind::NumberValue(100.0)),
        ]);
        let conf_kind = Kind::StructValue(kind_map);
        let conf: DataCollectorConfig = (&conf_kind).try_into()?;
        assert!(matches!(conf.method, CollectionMethod::Readings));
        assert_eq!(conf.capture_frequency_hz, 100.0);

        let kind_map = HashMap::from([
            (
                "method".to_string(),
                Kind::StringValue("AngularVelocity".to_string()),
            ),
            ("capture_frequency_hz".to_string(), Kind::NumberValue(100.0)),
        ]);
        let conf_kind = Kind::StructValue(kind_map);
        let conf: DataCollectorConfig = (&conf_kind).try_into()?;
        assert!(matches!(conf.method, CollectionMethod::AngularVelocity));
        assert_eq!(conf.capture_frequency_hz, 100.0);

        let kind_map = HashMap::from([
            (
                "method".to_string(),
                Kind::StringValue("MethodActing".to_string()),
            ),
            ("capture_frequency_hz".to_string(), Kind::NumberValue(100.0)),
        ]);
        let conf_kind = Kind::StructValue(kind_map);
        let conf_result = DataCollectorConfig::try_from(&conf_kind);
        assert!(matches!(
            conf_result,
            Err(AttributeError::ConversionImpossibleError)
        ));
        Ok(())
    }

    #[test_log::test]
    fn test_collect_data() -> Result<(), DataCollectionError> {
        let sensor = Arc::new(Mutex::new(FakeSensor::new()));
        let resource = ResourceType::Sensor(sensor);
        let kind_map = HashMap::from([
            (
                "method".to_string(),
                Kind::StringValue("Readings".to_string()),
            ),
            ("capture_frequency_hz".to_string(), Kind::NumberValue(100.0)),
        ]);
        let conf_kind = Kind::StructValue(kind_map);
        let conf =
            DataCollectorConfig::try_from(&conf_kind).expect("data collector config parse failed");
        let mut coll = DataCollector::from_config("fake".to_string(), resource, conf)?;
        let data = coll.call_method()?.data;
        assert!(data.is_some());
        let data = data.unwrap();
        match data {
            Data::Binary(_) => panic!("expected struct not binary data"),
            Data::Struct(d) => {
                let readings = d.fields.get("readings");
                assert!(readings.is_some());
                let readings = readings.unwrap();
                let readings = &readings.kind;
                assert!(readings.is_some());
                let readings = readings.clone().unwrap();
                let readings = match readings {
                    google::protobuf::value::Kind::StructValue(s) => s,
                    _ => panic!("readings was not a struct"),
                };
                let fake_reading = readings.fields.get("fake_sensor");
                assert!(fake_reading.is_some());
                let fake_reading = &fake_reading.clone().unwrap().kind;
                assert!(fake_reading.is_some());
                let fake_reading = fake_reading.clone().unwrap();
                match fake_reading {
                    google::protobuf::value::Kind::NumberValue(fake_reading) => {
                        assert_eq!(fake_reading, 42.42);
                    }
                    _ => panic!("fake reading was not a number"),
                };
            }
        };
        Ok(())
    }
}
