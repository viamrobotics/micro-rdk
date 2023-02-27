#![allow(dead_code)]

use crate::common::status::Status;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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

impl Status for FakeSensor {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}
