use std::fmt::Display;
use std::time::{Duration, Instant};

use crate::google::protobuf::Timestamp;
use crate::proto::app::data_sync::v1::{MimeType, SensorData, SensorMetadata};

use super::{
    config::{AttributeError, Kind},
    movement_sensor::MovementSensor,
    robot::ResourceType,
    sensor::{Readings, SensorError},
};

use thiserror::Error;

pub(crate) const DEFAULT_CACHE_SIZE_KB: f64 = 8.0;

/// A DataCollectorConfig instance is a representation of an element
/// of the list of "capture_methods" in the "attributes" section of a
/// component's configuration JSON object as stored in app. Each element
/// of "capture_methods" is meant to produce an instance of `DataCollector`
/// as defined below
#[derive(Debug, Clone)]
pub struct DataCollectorConfig {
    pub method: CollectionMethod,
    pub capture_frequency_hz: f32,
    pub capacity: usize,
    pub disabled: bool,
}

impl TryFrom<&Kind> for DataCollectorConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        let disabled = match value.get("disabled") {
            Ok(val) => match val {
                Some(Kind::BoolValue(v)) => *v,
                _ => false,
            },
            Err(_) => false,
        };
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
        let capacity_kb: f64 = value
            .get("cache_size_kb")?
            .unwrap_or(&Kind::NumberValue(DEFAULT_CACHE_SIZE_KB))
            .try_into()?;
        let capacity = (capacity_kb * 1000.0) as usize;
        if (capacity < 1000) && !disabled {
            return Err(AttributeError::ValidationError(
                "cache size must be at least 1KB".to_string(),
            ));
        }
        // TODO: RSDK-7127 - Collectors that take arguments (ex. Board Analogs)
        let method = match method_str.as_str() {
            "Readings" => CollectionMethod::Readings,
            "AngularVelocity" => CollectionMethod::AngularVelocity,
            "LinearAcceleration" => CollectionMethod::LinearAcceleration,
            "LinearVelocity" => CollectionMethod::LinearVelocity,
            _ => {
                return Err(AttributeError::ConversionImpossibleError);
            }
        };
        Ok(DataCollectorConfig {
            method,
            capture_frequency_hz,
            capacity,
            disabled,
        })
    }
}

/// A CollectionMethod is an enum whose values are associated with
/// a method on one or more component traits
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionMethod {
    Readings,
    // MovementSensor methods
    AngularVelocity,
    LinearAcceleration,
    LinearVelocity,
    // TODO: RSDK-7127 - Implement collectors for all other applicable components/methods
}

impl Display for CollectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // NOTE: Future added methods need to follow the same upper camel case
        // convention to not break downstream webhooks / database triggers
        std::fmt::Display::fmt(
            match self {
                Self::Readings => "Readings",
                Self::AngularVelocity => "AngularVelocity",
                Self::LinearAcceleration => "LinearAcceleration",
                Self::LinearVelocity => "LinearVelocity",
            },
            f,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceMethodKey {
    pub r_name: String,
    pub component_type: String,
    pub method: CollectionMethod,
}

impl Display for ResourceMethodKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ResourceMethodKey ({}:{}, {})",
            self.component_type, self.r_name, &self.method
        )
    }
}

#[derive(Error, Debug)]
pub enum DataCollectionError {
    #[error("method {0} unsupported for {1}")]
    UnsupportedMethod(CollectionMethod, String),
    #[error("no collection methods supported for component")]
    NoSupportedMethods,
    #[error("capture frequency cannot be 0.0")]
    UnsupportedCaptureFrequency,
    #[error(transparent)]
    SensorCollectionError(#[from] SensorError),
}

/// A DataCollector represents an association between a data collection method and
/// a ResourceType (i.e. SensorType & Readings, BoardType & Analogs) and the frequency at
/// which the results of the method should be stored.
pub struct DataCollector {
    name: String,
    component_type: String,
    resource: ResourceType,
    method: CollectionMethod,
    time_interval: Duration,
    capacity: usize,
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
        _ => false,
    }
}

impl DataCollector {
    pub fn new(
        name: String,
        resource: ResourceType,
        method: CollectionMethod,
        capture_frequency_hz: f32,
        capacity: usize,
    ) -> Result<Self, DataCollectionError> {
        if capture_frequency_hz == 0.0 {
            return Err(DataCollectionError::UnsupportedCaptureFrequency);
        }
        let time_interval_ms = (1000.0 / capture_frequency_hz) as u64;
        let time_interval = Duration::from_millis(time_interval_ms);
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
            time_interval,
            capacity,
        })
    }

    pub fn from_config(
        name: String,
        resource: ResourceType,
        conf: &DataCollectorConfig,
    ) -> Result<Self, DataCollectionError> {
        Self::new(
            name,
            resource,
            conf.method.clone(),
            conf.capture_frequency_hz,
            conf.capacity,
        )
    }

    pub fn name(&self) -> String {
        self.name.to_string()
    }

    pub fn component_type(&self) -> String {
        self.component_type.to_string()
    }

    pub fn time_interval(&self) -> Duration {
        self.time_interval
    }

    pub fn method_str(&self) -> String {
        self.method.to_string()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// calls the method associated with the collector and returns the resulting data
    pub(crate) fn call_method(
        &mut self,
        robot_start_time: Instant,
    ) -> Result<SensorData, DataCollectionError> {
        let reading_requested_ts = robot_start_time.elapsed();
        let data = match &mut self.resource {
            ResourceType::Sensor(ref mut res) => match self.method {
                CollectionMethod::Readings => res.get_generic_readings()?.into(),
                _ => {
                    return Err(DataCollectionError::UnsupportedMethod(
                        self.method.clone(),
                        "sensor".to_string(),
                    ))
                }
            },
            ResourceType::MovementSensor(ref mut res) => match self.method {
                CollectionMethod::Readings => res.get_generic_readings()?.into(),
                CollectionMethod::AngularVelocity => res
                    .get_angular_velocity()?
                    .to_data_struct("angular_velocity"),
                CollectionMethod::LinearAcceleration => res
                    .get_linear_acceleration()?
                    .to_data_struct("linear_acceleration"),
                CollectionMethod::LinearVelocity => {
                    res.get_linear_velocity()?.to_data_struct("linear_velocity")
                }
                #[allow(unreachable_patterns)]
                // TODO: RSDK-7127 - remove when methods for other components are implemented
                _ => {
                    return Err(DataCollectionError::UnsupportedMethod(
                        self.method.clone(),
                        "movement_sensor".to_string(),
                    ))
                }
            },
            _ => return Err(DataCollectionError::NoSupportedMethods),
        };
        let reading_received_ts = robot_start_time.elapsed();
        Ok(SensorData {
            metadata: Some(SensorMetadata {
                time_received: Some(Timestamp {
                    seconds: reading_received_ts.as_secs() as i64,
                    nanos: reading_received_ts.subsec_nanos() as i32,
                }),
                time_requested: Some(Timestamp {
                    seconds: reading_requested_ts.as_secs() as i64,
                    nanos: reading_requested_ts.subsec_nanos() as i32,
                }),
                annotations: None,
                mime_type: MimeType::Unspecified.into(),
            }),
            data: Some(data),
        })
    }

    pub fn resource_method_key(&self) -> ResourceMethodKey {
        ResourceMethodKey {
            r_name: self.name(),
            component_type: self.component_type(),
            method: self.method.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::{
        CollectionMethod, DataCollectionError, DataCollector, DataCollectorConfig,
        DEFAULT_CACHE_SIZE_KB,
    };
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
        assert_eq!(conf.capacity, (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize);
        assert!(!conf.disabled);

        let kind_map = HashMap::from([
            (
                "method".to_string(),
                Kind::StringValue("AngularVelocity".to_string()),
            ),
            ("capture_frequency_hz".to_string(), Kind::NumberValue(100.0)),
            ("cache_size_kb".to_string(), Kind::NumberValue(2.0)),
            ("disabled".to_string(), Kind::BoolValue(true)),
        ]);
        let conf_kind = Kind::StructValue(kind_map);
        let conf: DataCollectorConfig = (&conf_kind).try_into()?;
        assert!(matches!(conf.method, CollectionMethod::AngularVelocity));
        assert_eq!(conf.capture_frequency_hz, 100.0);
        assert_eq!(conf.capacity, 2000);
        assert!(conf.disabled);

        let kind_map = HashMap::from([
            (
                "method".to_string(),
                Kind::StringValue("AngularVelocity".to_string()),
            ),
            ("capture_frequency_hz".to_string(), Kind::NumberValue(100.0)),
            ("cache_size_kb".to_string(), Kind::NumberValue(2.0)),
            ("disabled".to_string(), Kind::BoolValue(false)),
        ]);
        let conf_kind = Kind::StructValue(kind_map);
        let conf: DataCollectorConfig = (&conf_kind).try_into()?;
        assert!(!conf.disabled);

        let kind_map = HashMap::from([
            (
                "method".to_string(),
                Kind::StringValue("AngularVelocity".to_string()),
            ),
            ("capture_frequency_hz".to_string(), Kind::NumberValue(100.0)),
            ("cache_size_kb".to_string(), Kind::NumberValue(0.5)),
        ]);
        let conf_kind = Kind::StructValue(kind_map);
        let conf = DataCollectorConfig::try_from(&conf_kind);
        assert!(conf.is_err());

        let kind_map = HashMap::from([
            (
                "method".to_string(),
                Kind::StringValue("AngularVelocity".to_string()),
            ),
            ("capture_frequency_hz".to_string(), Kind::NumberValue(100.0)),
            ("cache_size_kb".to_string(), Kind::NumberValue(0.5)),
            ("disabled".to_string(), Kind::BoolValue(true)),
        ]);
        let conf_kind = Kind::StructValue(kind_map);
        let conf = DataCollectorConfig::try_from(&conf_kind);
        assert!(conf.is_ok());
        let conf = conf.unwrap();
        assert!(conf.disabled);

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

        let kind_map = HashMap::from([
            (
                "method".to_string(),
                Kind::StringValue("Readings".to_string()),
            ),
            ("capture_frequency_hz".to_string(), Kind::NumberValue(0.0)),
        ]);
        let conf_kind = Kind::StructValue(kind_map);
        let conf: DataCollectorConfig = (&conf_kind).try_into()?;
        let sensor = Arc::new(Mutex::new(FakeSensor::new()));
        let resource = ResourceType::Sensor(sensor);
        let coll_create_attempt = DataCollector::from_config("fake".to_string(), resource, &conf);
        assert!(matches!(
            coll_create_attempt,
            Err(DataCollectionError::UnsupportedCaptureFrequency)
        ));
        Ok(())
    }

    #[test_log::test]
    fn test_collect_data() -> Result<(), DataCollectionError> {
        let robot_start_time = Instant::now();
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
        let mut coll = DataCollector::from_config("fake".to_string(), resource, &conf)?;
        assert_eq!(coll.time_interval(), Duration::from_millis(10));
        let data = coll.call_method(robot_start_time)?.data;
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
                let fake_reading = &fake_reading.unwrap().kind;
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
