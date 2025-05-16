use micro_rdk::common::{
    movement_sensor::COMPONENT_NAME as MsCompName,
    registry::{ComponentRegistry, RegistryError},
    sensor::COMPONENT_NAME as SensorCompName,
};

pub mod sensors;

pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    registry.register_sensor("depth", &sensors::DepthSensor::from_config)?;
    registry.register_dependency_getter(SensorCompName, "depth", &sensors::get_raw_sensor_key)?;
    registry.register_sensor("debug-pgn", &sensors::DebugPgnSensor::from_config)?;
    registry.register_dependency_getter(
        SensorCompName,
        "debug-pgn",
        &sensors::get_raw_sensor_key,
    )?;
    registry.register_movement_sensor(
        "boat-movement-sensor",
        &sensors::ViamboatMovementSensor::from_config,
    )?;
    registry.register_dependency_getter(
        MsCompName,
        "boat-movement-sensor",
        &sensors::get_raw_sensor_key,
    )?;
    registry.register_sensor("pgn-sensor", &sensors::PgnSensor::from_config)?;
    registry.register_dependency_getter(
        SensorCompName,
        "pgn-sensor",
        &sensors::get_raw_sensor_key,
    )?;
    Ok(())
}
