use micro_rdk::common::registry::{ComponentRegistry, RegistryError};

#[cfg(feature = "esp32")]
pub mod bme280_ulp;
#[cfg(feature = "esp32")]
pub mod free_heap_sensor;
pub mod moisture_sensor;
pub mod water_pump;
#[cfg(feature = "esp32")]
pub mod wifi_rssi_sensor;

pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    #[cfg(feature = "esp32")]
    free_heap_sensor::register_models(registry)?;
    moisture_sensor::register_models(registry)?;
    water_pump::register_models(registry)?;
    #[cfg(feature = "esp32")]
    wifi_rssi_sensor::register_models(registry)?;
    #[cfg(feature = "esp32")]
    bme280_ulp::register_models(registry)?;
    Ok(())
}
