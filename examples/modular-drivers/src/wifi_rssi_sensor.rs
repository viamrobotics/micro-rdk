use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use micro_rdk::{
    common::{
        config::ConfigType,
        registry::{ComponentRegistry, Dependency, RegistryError},
        sensor::{
            GenericReadingsResult, Readings, Sensor, SensorError, SensorResult, SensorT,
            SensorType, TypedReadingsResult,
        },
        status::{Status, StatusError},
    },
    esp32::esp_idf_svc::sys::{esp, esp_wifi_sta_get_ap_info, wifi_ap_record_t},
    DoCommand,
};

#[derive(DoCommand)]
pub struct WifiRSSISensor;

pub(crate) fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    registry.register_sensor("wifi-rssi", &WifiRSSISensor::from_config)?;
    log::debug!("wifi-rssi sensor registration ok");
    Ok(())
}

impl WifiRSSISensor {
    pub fn from_config(
        _cfg: ConfigType,
        _deps: Vec<Dependency>,
    ) -> Result<SensorType, SensorError> {
        log::debug!("wifi-rssi sensor instantiated from config");
        Ok(Arc::new(Mutex::new(Self {})))
    }
}

impl Sensor for WifiRSSISensor {}

impl Readings for WifiRSSISensor {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        Ok(self
            .get_readings()?
            .into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

impl SensorT<f64> for WifiRSSISensor {
    fn get_readings(&self) -> Result<TypedReadingsResult<f64>, SensorError> {
        log::debug!("wifi-rssi sensor - get readings called");
        let mut ap_info = wifi_ap_record_t::default();
        esp!(unsafe { esp_wifi_sta_get_ap_info(&mut ap_info as *mut wifi_ap_record_t) })
            .map_err(SensorError::EspError)?;
        let mut x = HashMap::new();
        x.insert("rssi".to_string(), ap_info.rssi as f64);
        log::debug!("wifi-rssi sensor - get readings OK");
        Ok(x)
    }
}

impl Status for WifiRSSISensor {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError> {
        log::debug!("wifi-rssi sensor - get status called");
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
