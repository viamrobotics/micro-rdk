#![allow(dead_code)]

use crate::common::status::Status;
use crate::google;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::analog::AnalogError;
use super::board::BoardError;
use super::config::ConfigType;
use super::generic::DoCommand;
use super::i2c::I2CErrors;
use super::registry::{ComponentRegistry, Dependency};
use thiserror::Error;
pub static COMPONENT_NAME: &str = "sensor";

#[derive(Debug, Error)]
pub enum SensorError {
    #[error(transparent)]
    AnalogError(#[from] AnalogError),
    #[error("sensor config error: {0}")]
    ConfigError(&'static str),
    #[error(transparent)]
    SensorI2CError(#[from] I2CErrors),
    #[error("{0}")]
    SensorGenericError(&'static str),
    #[error("method {0} unimplemented")]
    SensorMethodUnimplemented(&'static str),
    #[error(transparent)]
    SensorBoardError(#[from] BoardError),
    #[error("sensor error code {0}")]
    SensorCodeError(i32),
}

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_sensor("fake", &FakeSensor::from_config)
        .is_err()
    {
        log::error!("fake sensor type is already registered");
    }
}

pub type GenericReadingsResult =
    ::std::collections::HashMap<::prost::alloc::string::String, google::protobuf::Value>;

pub type TypedReadingsResult<T> = ::std::collections::HashMap<String, T>;

pub trait Readings {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError>;
}

pub trait Sensor: Readings + Status + DoCommand {}

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

#[derive(DoCommand)]
pub struct FakeSensor {
    fake_reading: f64,
}

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

impl Default for FakeSensor {
    fn default() -> Self {
        Self::new()
    }
}

impl Sensor for FakeSensor {}

impl Readings for FakeSensor {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        Ok(self
            .get_readings()?
            .into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

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
}

impl<A> Readings for Arc<Mutex<A>>
where
    A: ?Sized + Readings,
{
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        self.lock().unwrap().get_generic_readings()
    }
}

impl Status for FakeSensor {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
