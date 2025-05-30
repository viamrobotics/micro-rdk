#![allow(dead_code)]

#[cfg(feature = "builtin-components")]
use {
    super::config::ConfigType,
    super::registry::{ComponentRegistry, Dependency},
};

use crate::google;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::analog::AnalogError;
use super::board::BoardError;

use super::config::AttributeError;
use super::generic::DoCommand;
use super::i2c::I2CErrors;

use chrono::{DateTime, FixedOffset};
use thiserror::Error;

#[cfg(feature = "data")]
use crate::{
    google::protobuf::Timestamp,
    proto::app::data_sync::v1::{sensor_data::Data, MimeType, SensorData, SensorMetadata},
};

#[cfg(feature = "esp32")]
use crate::esp32::esp_idf_svc::sys::EspError;

pub static COMPONENT_NAME: &str = "sensor";

#[derive(Debug, Error)]
pub enum SensorError {
    #[error(transparent)]
    AnalogError(#[from] AnalogError),
    #[error(transparent)]
    ConfigAttributeError(#[from] AttributeError),
    #[error("sensor config error: {0}")]
    ConfigError(&'static str),
    #[error(transparent)]
    #[cfg(feature = "esp32")]
    EspError(#[from] EspError),
    #[error(transparent)]
    SensorI2CError(#[from] I2CErrors),
    #[error("{0}")]
    SensorGenericError(&'static str),
    #[error("{0}")]
    SensorDriverError(String),
    #[error("method {0} unimplemented")]
    SensorMethodUnimplemented(&'static str),
    #[error(transparent)]
    SensorBoardError(#[from] BoardError),
    #[error("sensor error code {0}")]
    SensorCodeError(i32),
}

#[cfg(feature = "builtin-components")]
pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_sensor("fake", &FakeSensor::from_config)
        .is_err()
    {
        log::error!("fake sensor type is already registered");
    }
}

pub type GenericReadingsResult = HashMap<::prost::alloc::string::String, google::protobuf::Value>;

#[cfg(feature = "data")]
impl From<GenericReadingsResult> for Data {
    fn from(value: GenericReadingsResult) -> Self {
        Data::Struct(google::protobuf::Struct {
            fields: HashMap::from([(
                "readings".to_string(),
                google::protobuf::Value {
                    kind: Some(google::protobuf::value::Kind::StructValue(
                        google::protobuf::Struct { fields: value },
                    )),
                },
            )]),
        })
    }
}

pub type TypedReadingsResult<T> = ::std::collections::HashMap<String, T>;

pub struct ReadingsTimestamp {
    pub reading_requested_dt: DateTime<FixedOffset>,
    pub reading_received_dt: DateTime<FixedOffset>,
}

pub trait Readings {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError>;
    #[cfg(feature = "data")]
    fn get_readings_data(&mut self) -> Result<SensorData, SensorError> {
        let reading_requested_dt = chrono::offset::Local::now().fixed_offset();
        let readings = self.get_generic_readings()?;
        let reading_received_dt = chrono::offset::Local::now().fixed_offset();

        Ok(SensorData {
            metadata: Some(SensorMetadata {
                time_received: Some(Timestamp {
                    seconds: reading_requested_dt.timestamp(),
                    nanos: reading_requested_dt.timestamp_subsec_nanos() as i32,
                }),
                time_requested: Some(Timestamp {
                    seconds: reading_received_dt.timestamp(),
                    nanos: reading_received_dt.timestamp_subsec_nanos() as i32,
                }),
                annotations: None,
                mime_type: MimeType::Unspecified.into(),
            }),
            data: Some(readings.into()),
        })
    }

    /// Optional
    fn get_cached_readings(
        &mut self,
    ) -> Result<Vec<(ReadingsTimestamp, GenericReadingsResult)>, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "sensor does not support independent readings caching",
        ))
    }

    #[cfg(feature = "data")]
    fn get_cached_readings_data(&mut self) -> Result<Vec<SensorData>, SensorError> {
        let cached_readings: Vec<SensorData> = self
            .get_cached_readings()?
            .into_iter()
            .map(|(ts, readings)| SensorData {
                metadata: Some(SensorMetadata {
                    time_received: Some(Timestamp {
                        seconds: ts.reading_requested_dt.timestamp(),
                        nanos: ts.reading_requested_dt.timestamp_subsec_nanos() as i32,
                    }),
                    time_requested: Some(Timestamp {
                        seconds: ts.reading_received_dt.timestamp(),
                        nanos: ts.reading_received_dt.timestamp_subsec_nanos() as i32,
                    }),
                    annotations: None,
                    mime_type: MimeType::Unspecified.into(),
                }),
                data: Some(readings.into()),
            })
            .collect();
        Ok(cached_readings)
    }
}

pub trait Sensor: Readings + DoCommand + Send {}

pub type SensorType = Arc<Mutex<dyn Sensor>>;

pub trait SensorT<T>: Sensor {
    fn get_readings(&self) -> Result<TypedReadingsResult<T>, SensorError>;
}

// A local wrapper type we can use to specialize `From` for `google::protobuf::Value``
pub struct SensorResult<T> {
    pub value: T,
}

impl From<SensorResult<f64>> for google::protobuf::Value {
    fn from(value: SensorResult<f64>) -> google::protobuf::Value {
        google::protobuf::Value {
            kind: Some(google::protobuf::value::Kind::NumberValue(value.value)),
        }
    }
}

#[cfg(feature = "builtin-components")]
#[derive(DoCommand)]
pub struct FakeSensor {
    fake_reading: f64,
}

#[cfg(feature = "builtin-components")]
impl FakeSensor {
    pub fn new() -> Self {
        FakeSensor {
            fake_reading: 42.42,
        }
    }
    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<SensorType, SensorError> {
        if let Ok(val) = cfg.get_attribute::<f64>("fake_value") {
            return Ok(Arc::new(Mutex::new(FakeSensor { fake_reading: val })));
        }
        Ok(Arc::new(Mutex::new(FakeSensor::new())))
    }
}

#[cfg(feature = "builtin-components")]
impl Default for FakeSensor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "builtin-components")]
impl Sensor for FakeSensor {}

#[cfg(feature = "builtin-components")]
impl Readings for FakeSensor {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        Ok(self
            .get_readings()?
            .into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

#[cfg(feature = "builtin-components")]
impl SensorT<f64> for FakeSensor {
    fn get_readings(&self) -> Result<TypedReadingsResult<f64>, SensorError> {
        let mut x = HashMap::new();
        x.insert("fake_sensor".to_string(), self.fake_reading);
        Ok(x)
    }
}

impl<A> Sensor for Mutex<A> where A: ?Sized + Sensor {}

impl<A> Sensor for Arc<Mutex<A>> where A: ?Sized + Sensor {}

impl<A> Readings for Mutex<A>
where
    A: ?Sized + Readings,
{
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        self.get_mut().unwrap().get_generic_readings()
    }
    fn get_cached_readings(
        &mut self,
    ) -> Result<Vec<(ReadingsTimestamp, GenericReadingsResult)>, SensorError> {
        self.get_mut().unwrap().get_cached_readings()
    }
}

impl<A> Readings for Arc<Mutex<A>>
where
    A: ?Sized + Readings,
{
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        self.lock().unwrap().get_generic_readings()
    }

    fn get_cached_readings(
        &mut self,
    ) -> Result<Vec<(ReadingsTimestamp, GenericReadingsResult)>, SensorError> {
        self.lock().unwrap().get_cached_readings()
    }
}
