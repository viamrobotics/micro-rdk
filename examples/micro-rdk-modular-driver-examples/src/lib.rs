use micro_rdk::common::registry::{ComponentRegistry, RegistryError};

pub mod moisture_sensor;
pub mod water_pump;

pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    moisture_sensor::register_models(registry)?;
    water_pump::register_models(registry)?;
    Ok(())
}
