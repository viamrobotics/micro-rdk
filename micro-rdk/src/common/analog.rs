#![allow(dead_code)]

use super::config::{AttributeError, Kind};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalogError {
    #[error("analog read error {0}")]
    AnalogReadError(i32),
}

pub struct FakeAnalogReader {
    name: String,
    value: u16,
}

impl FakeAnalogReader {
    pub fn new(name: String, value: u16) -> Self {
        Self { name, value }
    }
    fn internal_name(&self) -> String {
        self.name.clone()
    }
    fn internal_read(&self) -> Result<u16, AnalogError> {
        Ok(self.value)
    }
}

impl AnalogReader<u16> for FakeAnalogReader {
    type Error = AnalogError;
    fn name(&self) -> String {
        self.internal_name()
    }
    fn read(&mut self) -> Result<u16, Self::Error> {
        self.internal_read()
    }
}

pub trait AnalogReader<Word> {
    type Error;
    fn read(&mut self) -> Result<Word, Self::Error>;
    fn name(&self) -> String;
}

impl<A, Word> AnalogReader<Word> for Arc<Mutex<A>>
where
    A: ?Sized + AnalogReader<Word>,
{
    type Error = A::Error;
    fn read(&mut self) -> Result<Word, Self::Error> {
        self.lock().unwrap().read()
    }
    fn name(&self) -> String {
        self.lock().unwrap().name()
    }
}

pub(crate) struct AnalogReaderConfig {
    pub(crate) name: String,
    pub(crate) pin: i32,
}

impl TryFrom<&Kind> for AnalogReaderConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if !value.contains_key("name")? {
            return Err(AttributeError::KeyNotFound("name".to_string()));
        }
        if !value.contains_key("pin")? {
            return Err(AttributeError::KeyNotFound("pin".to_string()));
        }
        let name = value.get("name")?.unwrap().try_into()?;
        let pin: i32 = value.get("pin")?.unwrap().try_into()?;
        Ok(Self { name, pin })
    }
}

pub type AnalogReaderType<W, E = AnalogError> = Arc<Mutex<dyn AnalogReader<W, Error = E>>>;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::config::{Component, DynamicComponentConfig, Kind};

    use super::AnalogReaderConfig;
    #[test_log::test]
    fn test_analog_reader_config() {
        let robot_config: &[DynamicComponentConfig] = &[DynamicComponentConfig {
            name: "board".to_owned(),
            namespace: "rdk".to_owned(),
            r#type: "board".to_owned(),
            model: "fake".to_owned(),
            attributes: Some(HashMap::from([
                (
                    "pins".to_owned(),
                    Kind::VecValue(vec![
                        Kind::StringValue("11".to_owned()),
                        Kind::StringValue("12".to_owned()),
                        Kind::StringValue("13".to_owned()),
                    ]),
                ),
                (
                    "analogs".to_owned(),
                    Kind::VecValue(vec![
                        Kind::StructValue(HashMap::from([
                            ("name".to_owned(), Kind::StringValue("string".to_owned())),
                            ("pin".to_owned(), Kind::StringValue("12".to_owned())),
                        ])),
                        Kind::StructValue(HashMap::from([
                            ("name".to_owned(), Kind::StringValue("string".to_owned())),
                            ("pin".to_owned(), Kind::StringValue("11".to_owned())),
                        ])),
                    ]),
                ),
            ])),
        }];

        let val = robot_config[0].get_attribute::<Vec<AnalogReaderConfig>>("analogs");

        assert!(&val.is_ok());

        let val = val.unwrap();

        assert_eq!(val.len() as u32, 2);

        assert_eq!(val[0].name, "string");
        assert_eq!(val[1].name, "string");
        assert_eq!(val[0].pin, 12);
        assert_eq!(val[1].pin, 11);
    }
}
