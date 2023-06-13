#![allow(dead_code)]

use crate::common::status::Status;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::board::BoardType;
use super::config::ConfigType;
use super::registry::ComponentRegistry;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_sensor("fake", &FakeSensor::from_config)
        .is_err()
    {
        log::error!("fake sensor type is already registered");
    }
}

pub type GenericReadingsResult =
    ::std::collections::HashMap<::prost::alloc::string::String, ::prost_types::Value>;

pub type TypedReadingsResult<T> = ::std::collections::HashMap<String, T>;

pub trait Sensor: Status {
    fn get_generic_readings(&self) -> anyhow::Result<GenericReadingsResult>;
}

pub(crate) type SensorType = Arc<Mutex<dyn Sensor>>;

pub trait SensorT<T>: Sensor {
    fn get_readings(&self) -> anyhow::Result<TypedReadingsResult<T>>;
}

// A local wrapper type we can use to specialize `From` for `prost_types::Value``
pub struct SensorResult<T> {
    pub value: T,
}

impl From<SensorResult<f64>> for ::prost_types::Value {
    fn from(value: SensorResult<f64>) -> ::prost_types::Value {
        prost_types::Value {
            kind: Some(::prost_types::value::Kind::NumberValue(value.value)),
        }
    }
}

pub struct FakeSensor {
    fake_reading: f64,
}

impl FakeSensor {
    pub fn new() -> Self {
        FakeSensor {
            fake_reading: 42.42,
        }
    }
    pub(crate) fn from_config(cfg: ConfigType, _: Option<BoardType>) -> anyhow::Result<SensorType> {
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

impl Sensor for FakeSensor {
    fn get_generic_readings(&self) -> anyhow::Result<GenericReadingsResult> {
        Ok(self
            .get_readings()?
            .into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

impl SensorT<f64> for FakeSensor {
    fn get_readings(&self) -> anyhow::Result<TypedReadingsResult<f64>> {
        let mut x = HashMap::new();
        x.insert("fake_sensor".to_string(), self.fake_reading);
        Ok(x)
    }
}
impl<A> Sensor for Mutex<A>
where
    A: ?Sized + Sensor,
{
    fn get_generic_readings(&self) -> anyhow::Result<GenericReadingsResult> {
        self.lock().unwrap().get_generic_readings()
    }
}

impl Status for FakeSensor {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}
