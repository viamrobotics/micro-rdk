use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use micro_rdk::{
    common::{
        config::ConfigType,
        registry::{ComponentRegistry, Dependency, RegistryError},
        sensor::{
            GenericReadingsResult, Readings, Sensor, SensorResult, SensorT, SensorType,
            TypedReadingsResult, SensorError
        },
        status::{Status, StatusError},
    },
    esp32::esp_idf_svc::sys::esp_get_free_heap_size,
    DoCommand,
};

#[derive(DoCommand)]
pub struct FreeHeapSensor;

pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    registry.register_sensor("free-heap", &FreeHeapSensor::from_config)?;
    log::debug!("free-heap sensor registration ok");
    Ok(())
}

impl FreeHeapSensor {
    pub fn from_config(_cfg: ConfigType, _deps: Vec<Dependency>) -> Result<SensorType, SensorError> {
        log::debug!("free-heap sensor instantiated from config");
        Ok(Arc::new(Mutex::new(Self {})))
    }
}

impl Sensor for FreeHeapSensor {}
impl Readings for FreeHeapSensor {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        Ok(self
            .get_readings()?
            .into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

impl SensorT<f64> for FreeHeapSensor {
    fn get_readings(&self) -> Result<TypedReadingsResult<f64>, SensorError> {
        log::debug!("free-heap sensor - get readings called");
        let reading = unsafe { esp_get_free_heap_size() };
        let mut x = HashMap::new();
        x.insert("bytes".to_string(), reading as f64);
        log::debug!("free-heap sensor - get readings OK");
        Ok(x)
    }
}

impl Status for FreeHeapSensor {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError>  {
        log::debug!("free-heap sensor - get status called");
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
