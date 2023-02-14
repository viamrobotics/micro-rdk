#![allow(dead_code)]

#[derive(Debug, Eq, PartialEq)]
pub enum AttributeError {
    ParseNumError,
    ConversionImpossibleError,
    KeyNotFound,
}

use std::{
    collections::BTreeMap,
    num::{ParseFloatError, ParseIntError},
};

use crate::proto::common::v1::ResourceName;

impl From<ParseIntError> for AttributeError {
    fn from(_: ParseIntError) -> AttributeError {
        AttributeError::ParseNumError
    }
}
impl From<ParseFloatError> for AttributeError {
    fn from(_: ParseFloatError) -> AttributeError {
        AttributeError::ParseNumError
    }
}

macro_rules! primitives
{
    ( $($t:ty),* ) =>
    {
        $(impl TryFrom<Kind> for $t
          {
              type Error = AttributeError;
              fn try_from(value: Kind) -> Result<Self, Self::Error> {
                  match value {
                      Kind::NullValue(v) => Ok(v as $t),
                      Kind::NumberValue(v) => Ok(v as $t),
                      Kind::BoolValue(v) => Ok(v as $t),
                      Kind::StringValueStatic(v) => Ok(v.parse::<$t>()?),
                      _ => Err(AttributeError::ConversionImpossibleError),
                  }
              }
          }
          impl TryFrom<&Kind> for $t
          {
              type Error = AttributeError;
              fn try_from(value: &Kind) -> Result<Self, Self::Error> {
                  match value {
                      Kind::NullValue(v) => Ok(*v as $t),
                      Kind::NumberValue(v) => Ok(*v as $t),
                      Kind::BoolValue(v) => Ok(*v as $t),
                      Kind::StringValueStatic(v) => Ok(v.parse::<$t>()?),
                      _ => Err(AttributeError::ConversionImpossibleError),
                  }
              }
          }
        )*
    }
}
primitives!(u32, i32, u8, u16, i16, i8);

macro_rules! floats
{
    ( $($t:ty),* ) =>
    {
        $(impl TryFrom<Kind> for $t
          {
              type Error = AttributeError;
              fn try_from(value: Kind) -> Result<Self, Self::Error> {
                  match value {
                      Kind::NullValue(v) => Ok(v as $t),
                      Kind::NumberValue(v) => Ok(v as $t),
                      Kind::StringValueStatic(v) => Ok(v.parse::<$t>()?),
                      _ => Err(AttributeError::ConversionImpossibleError),
                  }
              }
          }
          impl TryFrom<&Kind> for $t
          {
              type Error = AttributeError;
              fn try_from(value: &Kind) -> Result<Self, Self::Error> {
                  match value {
                      Kind::NullValue(v) => Ok(*v as $t),
                      Kind::NumberValue(v) => Ok(*v as $t),
                      Kind::StringValueStatic(v) => Ok(v.parse::<$t>()?),
                      _ => Err(AttributeError::ConversionImpossibleError),
                  }
              }
          }
        )*
    }
}

floats!(f64, f32);

impl<V> TryFrom<Kind> for BTreeMap<&'static str, V>
where
    V: for<'a> std::convert::TryFrom<&'a Kind, Error = AttributeError>,
{
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StructValueStatic(v) => v
                .into_iter()
                .map(|(k, v)| Ok((*k, v.try_into()?)))
                .collect(),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}
impl<V> TryFrom<&Kind> for BTreeMap<&'static str, V>
where
    V: for<'a> std::convert::TryFrom<&'a Kind, Error = AttributeError>,
{
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        println!("Obj {:?}", value);
        match value {
            Kind::StructValueStatic(v) => v
                .into_iter()
                .map(|(k, v)| Ok((*k, v.try_into()?)))
                .collect(),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl<T> TryFrom<&Kind> for Vec<T>
where
    T: for<'a> std::convert::TryFrom<&'a Kind, Error = AttributeError>,
{
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::ListValueStatic(v) => v.iter().map(|v| v.try_into()).collect(),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl<T> TryFrom<Kind> for Vec<T>
where
    T: for<'a> std::convert::TryFrom<&'a Kind, Error = AttributeError>,
{
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::ListValueStatic(v) => v.iter().map(|v| v.try_into()).collect(),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<&Kind> for Kind {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::BoolValue(v) => Ok(Kind::BoolValue(*v)),
            Kind::NullValue(v) => Ok(Kind::NullValue(*v)),
            Kind::StringValueStatic(v) => Ok(Kind::StringValueStatic(v)),
            Kind::NumberValue(v) => Ok(Kind::NumberValue(*v)),
            Kind::ListValueStatic(v) => Ok(Kind::ListValueStatic(v)),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

#[derive(Debug)]
pub enum Kind {
    NullValue(i32),
    NumberValue(f64),
    StringValueStatic(&'static str),
    BoolValue(bool),
    StructValueStatic(phf::map::Map<&'static str, Kind>),
    ListValueStatic(&'static [Kind]),
}

#[derive(Debug, Default)]
pub struct StaticComponentConfig {
    pub name: &'static str,
    pub namespace: &'static str,
    pub r#type: &'static str,
    pub model: &'static str,
    pub attributes: Option<phf::map::Map<&'static str, Kind>>,
}

#[derive(Debug)]
pub struct RobotConfigStatic {
    pub components: Option<&'static [StaticComponentConfig]>,
}

pub trait Component {
    fn get_name(&self) -> &str;
    fn get_model(&self) -> &str;
    fn get_type(&self) -> &str;
    fn get_namespace(&self) -> &str;
    fn get_resource_name(&self) -> ResourceName {
        ResourceName {
            namespace: self.get_namespace().to_string(),
            r#type: "component".to_string(),
            subtype: self.get_type().to_string(),
            name: self.get_name().to_string(),
        }
    }
    fn get_attribute<'a, T>(&'a self, key: &str) -> Result<T, AttributeError>
    where
        T: std::convert::TryFrom<Kind, Error = AttributeError>
            + std::convert::TryFrom<&'a Kind, Error = AttributeError>;
}

impl Component for StaticComponentConfig {
    fn get_name(&self) -> &str {
        self.name
    }
    fn get_model(&self) -> &str {
        self.model
    }
    fn get_type(&self) -> &str {
        self.r#type
    }
    fn get_namespace(&self) -> &str {
        self.namespace
    }
    fn get_attribute<'a, T>(&'a self, key: &str) -> Result<T, AttributeError>
    where
        T: std::convert::TryFrom<Kind, Error = AttributeError>
            + std::convert::TryFrom<&'a Kind, Error = AttributeError>,
    {
        if let Some(v) = self.attributes.as_ref() {
            if let Some(v) = v.get(key) {
                return v.try_into();
            }
        }
        Err(AttributeError::KeyNotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::common::config::{
        AttributeError, Component, Kind, RobotConfigStatic, StaticComponentConfig,
    };
    #[test_log::test]
    fn test_config_component() -> anyhow::Result<()> {
        #[allow(clippy::redundant_static_lifetimes, dead_code)]
        const PMR: &'static RobotConfigStatic = &RobotConfigStatic {
            components: Some(&[
                StaticComponentConfig {
                    name: "board",
                    namespace: "rdk",
                    r#type: "board",
                    model: "pi",
                    attributes: Some(
                        phf::phf_map! {"pins" => Kind::ListValueStatic(&[Kind::StringValueStatic("11"),Kind::StringValueStatic("12"),Kind::StringValueStatic("13")])},
                    ),
                },
                StaticComponentConfig {
                    name: "motor",
                    namespace: "rdk",
                    r#type: "motor",
                    model: "gpio",
                    attributes: Some(
                        phf::phf_map! {"pins" => Kind::StructValueStatic(phf::phf_map!{"a" => Kind::StringValueStatic("29"),"pwm" => Kind::StringValueStatic("12"),"b" => Kind::StringValueStatic("5")}),"board" => Kind::StringValueStatic("board")},
                    ),
                },
                StaticComponentConfig {
                    name: "motor",
                    namespace: "rdk",
                    r#type: "motor",
                    model: "gpio",
                    attributes: Some(
                        phf::phf_map! {"float" => Kind::NumberValue(10.556), "float2" => Kind::StringValueStatic("10.564"), "float3" => Kind::StringValueStatic("-1.18e+11"),
                        "pins" => Kind::ListValueStatic(&[Kind::StringValueStatic("11000"),Kind::StringValueStatic("12"),Kind::StringValueStatic("13")]),
                        "pins2" => Kind::StructValueStatic(phf::phf_map!{"a" => Kind::StringValueStatic("29000")})},
                    ),
                },
            ]),
        };

        let val = PMR.components.unwrap()[0].get_name();
        assert_eq!(val, "board");

        let val = PMR.components.unwrap()[0].get_model();
        assert_eq!(val, "pi");

        let val = PMR.components.unwrap()[0].get_type();
        assert_eq!(val, "board");

        let val = PMR.components.unwrap()[1].get_name();
        assert_eq!(val, "motor");

        let val = PMR.components.unwrap()[1].get_model();
        assert_eq!(val, "gpio");

        let val = PMR.components.unwrap()[1].get_type();
        assert_eq!(val, "motor");

        let val = PMR.components.unwrap()[1].get_attribute::<u32>("board");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(val.err().unwrap(), AttributeError::ParseNumError);

        let val = PMR.components.unwrap()[1].get_attribute::<u32>("nope");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(val.err().unwrap(), AttributeError::KeyNotFound);

        let val = PMR.components.unwrap()[0].get_attribute::<u32>("pins");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(
            val.err().unwrap(),
            AttributeError::ConversionImpossibleError
        );

        let val = PMR.components.unwrap()[2].get_attribute::<f32>("float");

        assert_eq!(val.as_ref().err(), None);
        assert_eq!(val.ok().unwrap(), 10.556);

        let val = PMR.components.unwrap()[2].get_attribute::<f32>("float2");

        assert_eq!(val.as_ref().err(), None);
        assert_eq!(val.ok().unwrap(), 10.564);

        let val = PMR.components.unwrap()[2].get_attribute::<f64>("float3");

        assert_eq!(val.as_ref().err(), None);
        assert_eq!(val.ok().unwrap(), -1.18e+11);

        let val = PMR.components.unwrap()[0].get_attribute::<u32>("pins");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(
            val.err().unwrap(),
            AttributeError::ConversionImpossibleError
        );

        let val = PMR.components.unwrap()[0].get_attribute::<Vec<i8>>("pins");

        assert_eq!(val.ok().unwrap(), vec![11, 12, 13]);

        let val = PMR.components.unwrap()[2].get_attribute::<Vec<i8>>("pins");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(val.err().unwrap(), AttributeError::ParseNumError);

        let val = PMR.components.unwrap()[1].get_attribute::<BTreeMap<&'static str, u32>>("pins");

        assert_eq!(val.as_ref().err(), None);
        let val = val.unwrap();

        assert_eq!(val["pwm"], 12);
        assert_eq!(val["a"], 29);
        assert_eq!(val["b"], 5);

        let val = PMR.components.unwrap()[2].get_attribute::<BTreeMap<&'static str, u8>>("pins2");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(val.err().unwrap(), AttributeError::ParseNumError);
        Ok(())
    }
}
