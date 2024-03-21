use crate::common::sensor::GenericReadingsResult;
use crate::common::sensor::Sensor;
use crate::common::sensor::SensorResult;
use crate::common::sensor::SensorT;
use crate::common::sensor::TypedReadingsResult;
use crate::common::status::Status;
use crate::common::status::StatusError;
use crate::google;

use std::collections::HashMap;

use super::analog::AnalogReaderType;
use super::sensor::Readings;
use super::sensor::SensorError;

#[derive(DoCommand)]
pub struct MoistureSensor {
    analog: AnalogReaderType<u16>,
}

impl MoistureSensor {
    pub fn new(analog: AnalogReaderType<u16>) -> Self {
        MoistureSensor { analog }
    }
}

impl Sensor for MoistureSensor {}

impl Readings for MoistureSensor {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        Ok(self
            .get_readings()?
            .into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

impl SensorT<f64> for MoistureSensor {
    fn get_readings(&self) -> Result<TypedReadingsResult<f64>, SensorError> {
        let reading = self.analog.lock().unwrap().read()?;
        let mut x = HashMap::new();
        x.insert("millivolts".to_string(), reading as f64);
        Ok(x)
    }
}

impl Status for MoistureSensor {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
