use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use micro_rdk::{
    common::{
        analog::{AnalogError, AnalogReader, AnalogReaderType},
        board::Board,
        config::ConfigType,
        registry::{get_board_from_dependencies, ComponentRegistry, Dependency, RegistryError},
        sensor::{GenericReadingsResult, Readings, Sensor, SensorError, SensorResult, SensorType},
        status::{Status, StatusError},
    },
    google::protobuf,
    DoCommand,
};

pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    registry.register_sensor("moisture_sensor", &MoistureSensor::from_config)?;
    Ok(())
}

#[derive(DoCommand)]
pub struct MoistureSensor<T: AnalogReader<u16, Error = AnalogError>> {
    reader: T,
}

impl MoistureSensor<AnalogReaderType<u16>> {
    pub fn from_config(_cfg: ConfigType, deps: Vec<Dependency>) -> Result<SensorType, SensorError> {
        let board = get_board_from_dependencies(deps);
        if board.is_none() {
            return Err(SensorError::ConfigError("sensor missing board attribute"));
        }
        let board_unwrapped = board.unwrap();
        if let Ok(reader) = board_unwrapped.get_analog_reader_by_name("moisture".to_string()) {
            Ok(Arc::new(Mutex::new(Self { reader })))
        } else {
            Err(SensorError::ConfigError(
                "failed to get analog reader `moisture`",
            ))
        }
    }
}

impl<T: AnalogReader<u16, Error = AnalogError>> Sensor for MoistureSensor<T> {}

impl<T: AnalogReader<u16, Error = AnalogError>> Readings for MoistureSensor<T> {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        let mut x: HashMap<String, f64> = HashMap::new();
        let reading = self.reader.read()?;
        x.insert("millivolts".to_string(), reading as f64);
        Ok(x.into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

impl<T: AnalogReader<u16, Error = AnalogError>> Status for MoistureSensor<T> {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError> {
        Ok(Some(protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
