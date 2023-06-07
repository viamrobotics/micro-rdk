#![allow(dead_code)]

use super::config::Kind;
use super::error::AttributeError;

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
    fn internal_read(&self) -> anyhow::Result<u16> {
        Ok(self.value)
    }
}

impl AnalogReader<u16> for FakeAnalogReader {
    type Error = anyhow::Error;
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

#[derive(Debug)]
pub(crate) struct AnalogReaderConfig {
    pub(crate) name: &'static str,
    pub(crate) pin: i32,
}

impl TryFrom<Kind> for AnalogReaderConfig {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StructValueStatic(v) => {
                if !v.contains_key("name") {
                    return Err(AttributeError::KeyNotFound);
                }
                if !v.contains_key("pin") {
                    return Err(AttributeError::KeyNotFound);
                }
                let name = v.get("name").unwrap().try_into()?;
                let pin: i32 = v.get("pin").unwrap().try_into()?;
                Ok(Self { name, pin })
            }
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<&Kind> for AnalogReaderConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StructValueStatic(v) => {
                if !v.contains_key("name") {
                    return Err(AttributeError::KeyNotFound);
                }
                if !v.contains_key("pin") {
                    return Err(AttributeError::KeyNotFound);
                }
                let name = v.get("name").unwrap().try_into()?;
                let pin: i32 = v.get("pin").unwrap().try_into()?;
                Ok(Self { name, pin })
            }
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::common::config::{Component, Kind, RobotConfigStatic, StaticComponentConfig};

    use super::AnalogReaderConfig;
    #[test_log::test]
    fn test_analog_reader_config() -> anyhow::Result<()> {
        #[allow(clippy::redundant_static_lifetimes, dead_code)]
        const STATIC_ROBOT_CONFIG: Option<RobotConfigStatic> = Some(RobotConfigStatic {
            components: Some(&[StaticComponentConfig {
                name: "board",
                namespace: "rdk",
                r#type: "board",
                model: "fake",
                attributes: Some(
                    phf::phf_map! {"pins" => Kind::ListValueStatic(&[Kind::StringValueStatic("11"),Kind::StringValueStatic("12"),Kind::StringValueStatic("13")]),"analogs" => Kind::ListValueStatic(&[Kind::StructValueStatic(phf::phf_map!{"name" => Kind::StringValueStatic("string"),"pi\
                    n" => Kind::StringValueStatic("12")}),Kind::StructValueStatic(phf::phf_map!{"name" => Kind::StringValueStatic("string"),"pin" => Kind::StringValueStatic("11")})])},
                ),
            }]),
        });
        let val = STATIC_ROBOT_CONFIG.unwrap().components.unwrap()[0]
            .get_attribute::<Vec<AnalogReaderConfig>>("analogs");

        assert!(&val.is_ok());

        let val = val.unwrap();

        assert_eq!(*&val.len() as u32, 2);

        assert_eq!(val[0].name, "string");
        assert_eq!(val[1].name, "string");
        assert_eq!(val[0].pin, 12);
        assert_eq!(val[1].pin, 11);
        Ok(())
    }
}
