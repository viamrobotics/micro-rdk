#![allow(dead_code)]
use crate::google;
use crate::proto::{app::v1::ComponentConfig, common::v1::ResourceName};

use std::collections::HashMap;
use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum AttributeError {
    #[error("failed to parse number")]
    ParseNumError,
    #[error("value not possible")]
    ConversionImpossibleError,
    #[error("attribute `{0}` was not found")]
    KeyNotFound(String),
}

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
                      Kind::StringValue(v) => Ok(v.parse::<$t>()?),
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
                      Kind::StringValue(v) => Ok(v.parse::<$t>()?),
                      _ => Err(AttributeError::ConversionImpossibleError),
                  }
              }
          }
        )*
    }
}
primitives!(u32, i32, u8, u16, i16, i8, u64);

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
                      Kind::StringValue(v) => Ok(v.parse::<$t>()?),
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
                      Kind::StringValue(v) => Ok(v.parse::<$t>()?),
                      _ => Err(AttributeError::ConversionImpossibleError),
                  }
              }
          }
        )*
    }
}

floats!(f64, f32);

impl<V> TryFrom<Kind> for HashMap<&'static str, V>
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
impl<V> TryFrom<&Kind> for HashMap<&'static str, V>
where
    V: for<'a> std::convert::TryFrom<&'a Kind, Error = AttributeError>,
{
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
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
            Kind::VecValue(v) => v.iter().map(|v| v.try_into()).collect(),
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
            Kind::VecValue(v) => v.iter().map(|v| v.try_into()).collect(),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<Kind> for &str {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StringValueStatic(v) => Ok(v),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<&Kind> for &str {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StringValueStatic(v) => Ok(*v),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<Kind> for String {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StringValue(v) => Ok(v),
            Kind::StringValueStatic(v) => Ok(v.to_owned()),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<&Kind> for String {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StringValue(v) => Ok(v.to_string()),
            Kind::StringValueStatic(v) => Ok((*v).to_owned()),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<Kind> for bool {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::BoolValue(v) => Ok(v),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<&Kind> for bool {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::BoolValue(v) => Ok(*v),
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
            Kind::VecValue(v) => {
                let mut v_copy = vec![];
                for k in v.iter() {
                    v_copy.push(k.try_into()?);
                }
                Ok(Kind::VecValue(v_copy))
            }
            Kind::StringValue(v) => Ok(Kind::StringValue(v.to_string())),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

#[derive(Debug)]
pub enum Kind {
    NullValue(i32),
    NumberValue(f64),
    StringValueStatic(&'static str),
    StringValue(String),
    BoolValue(bool),
    StructValueStatic(phf::map::Map<&'static str, Kind>),
    ListValueStatic(&'static [Kind]),
    VecValue(Vec<Kind>),
    StructValue(HashMap<String, Kind>),
}

impl Kind {
    pub fn get(&self, key: &str) -> Result<Option<&Kind>, AttributeError> {
        match self {
            Self::StructValueStatic(v) => Ok(v.get(key)),
            Self::StructValue(v) => Ok(v.get(key)),
            _ => Err(AttributeError::KeyNotFound(key.to_string())),
        }
    }

    pub fn contains_key(&self, key: &str) -> Result<bool, AttributeError> {
        match self {
            Self::StructValueStatic(v) => Ok(v.contains_key(key)),
            Self::StructValue(v) => Ok(v.contains_key(key)),
            _ => Err(AttributeError::KeyNotFound(key.to_string())),
        }
    }
}

impl TryFrom<google::protobuf::value::Kind> for Kind {
    type Error = AttributeError;
    fn try_from(value: google::protobuf::value::Kind) -> Result<Self, Self::Error> {
        match value {
            google::protobuf::value::Kind::BoolValue(v) => Ok(Kind::BoolValue(v)),
            google::protobuf::value::Kind::NullValue(v) => Ok(Kind::NullValue(v)),
            google::protobuf::value::Kind::StringValue(v) => Ok(Kind::StringValue(v)),
            google::protobuf::value::Kind::NumberValue(v) => Ok(Kind::NumberValue(v)),
            google::protobuf::value::Kind::StructValue(v) => {
                let mut attr_map = HashMap::new();
                let attrs = &v.fields;
                for (k, val) in attrs.iter() {
                    let k_copy = k.to_string();
                    match &val.kind {
                        Some(unwrapped) => {
                            attr_map.insert(k_copy, unwrapped.try_into()?);
                        }
                        None => continue,
                    };
                }
                Ok(Kind::StructValue(attr_map))
            }
            google::protobuf::value::Kind::ListValue(v) => {
                let try_mapped: Result<Vec<Kind>, AttributeError> = v
                    .values
                    .iter()
                    .map(|val| match &val.kind {
                        None => Ok::<Kind, AttributeError>(Kind::NullValue(0)),
                        Some(unwrapped) => Ok(Kind::try_from(unwrapped)?),
                    })
                    .collect();
                let mapped = try_mapped?;
                Ok(Kind::VecValue(mapped))
            }
        }
    }
}

impl TryFrom<&google::protobuf::value::Kind> for Kind {
    type Error = AttributeError;
    fn try_from(value: &google::protobuf::value::Kind) -> Result<Self, Self::Error> {
        match value {
            google::protobuf::value::Kind::BoolValue(v) => Ok(Kind::BoolValue(*v)),
            google::protobuf::value::Kind::NullValue(v) => Ok(Kind::NullValue(*v)),
            google::protobuf::value::Kind::StringValue(v) => {
                let v_copy = v.to_string();
                Ok(Kind::StringValue(v_copy))
            }
            google::protobuf::value::Kind::NumberValue(v) => Ok(Kind::NumberValue(*v)),
            google::protobuf::value::Kind::StructValue(v) => {
                let mut attr_map = HashMap::new();
                let attrs = &v.fields;
                for (k, val) in attrs.iter() {
                    match &val.kind {
                        Some(unwrapped) => {
                            let s = k.to_string();
                            attr_map.insert(s, unwrapped.try_into()?);
                        }
                        None => continue,
                    };
                }
                Ok(Kind::StructValue(attr_map))
            }
            google::protobuf::value::Kind::ListValue(v) => {
                let try_mapped: Result<Vec<Kind>, AttributeError> = v
                    .values
                    .iter()
                    .map(|val| match &val.kind {
                        None => Ok::<Kind, AttributeError>(Kind::NullValue(0)),
                        Some(unwrapped) => Ok(Kind::try_from(unwrapped)?),
                    })
                    .collect();
                let mapped = try_mapped?;
                Ok(Kind::VecValue(mapped))
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct StaticComponentConfig {
    pub name: &'static str,
    pub namespace: &'static str,
    pub r#type: &'static str,
    pub model: &'static str,
    pub attributes: Option<phf::map::Map<&'static str, Kind>>,
}

#[derive(Debug, Default)]
pub struct DynamicComponentConfig {
    pub name: String,
    pub namespace: String,
    pub r#type: String,
    pub model: String,
    pub attributes: Option<HashMap<String, Kind>>,
}

impl TryFrom<&ComponentConfig> for DynamicComponentConfig {
    type Error = AttributeError;
    fn try_from(value: &ComponentConfig) -> Result<Self, Self::Error> {
        let mut attrs_opt: Option<HashMap<String, Kind>> = None;
        if let Some(cfg_attrs) = value.attributes.as_ref() {
            let mut attrs = HashMap::new();
            for (k, v) in cfg_attrs.fields.iter() {
                let val: Kind = match &v.kind {
                    None => return Err(AttributeError::KeyNotFound(k.to_string())),
                    Some(inner_v) => inner_v.try_into()?,
                };
                let key = k.to_string();
                attrs.insert(key, val);
            }
            attrs_opt = Some(attrs);
        }
        Ok(Self {
            name: value.name.to_string(),
            namespace: value.namespace.to_string(),
            r#type: value.r#type.to_string(),
            model: value.model.to_string(),
            attributes: attrs_opt,
        })
    }
}

#[derive(Debug)]
pub enum ConfigType {
    Static(&'static StaticComponentConfig),
    Dynamic(DynamicComponentConfig),
}

impl ConfigType {
    pub fn get_attribute<T>(&self, key: &str) -> Result<T, AttributeError>
    where
        for<'a> T: std::convert::TryFrom<Kind, Error = AttributeError>
            + std::convert::TryFrom<&'a Kind, Error = AttributeError>,
    {
        match self {
            Self::Static(cfg) => cfg.get_attribute::<T>(key),
            Self::Dynamic(cfg) => cfg.get_attribute::<T>(key),
        }
    }
    pub fn get_type(&self) -> &str {
        match self {
            Self::Static(cfg) => cfg.get_type(),
            Self::Dynamic(cfg) => cfg.get_type(),
        }
    }
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
        self.attributes
            .as_ref()
            .ok_or_else(|| AttributeError::KeyNotFound(key.to_owned()))? // no attribute map
            .get(key)
            .ok_or_else(|| AttributeError::KeyNotFound(key.to_owned()))? // no key in attribute map
            .try_into()
    }
}

impl Component for DynamicComponentConfig {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_model(&self) -> &str {
        &self.model
    }

    fn get_namespace(&self) -> &str {
        &self.namespace
    }

    fn get_type(&self) -> &str {
        &self.r#type
    }

    fn get_attribute<'a, T>(&'a self, key: &str) -> Result<T, AttributeError>
    where
        T: std::convert::TryFrom<Kind, Error = AttributeError>
            + std::convert::TryFrom<&'a Kind, Error = AttributeError>,
    {
        if let Some(v) = self.attributes.as_ref() {
            let key_string = key.to_owned();
            if let Some(v) = v.get(&key_string) {
                return v.try_into();
            }
        }
        Err(AttributeError::KeyNotFound(key.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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
        assert_eq!(
            val.err().unwrap(),
            AttributeError::KeyNotFound("nope".to_string())
        );

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

        let val = PMR.components.unwrap()[1].get_attribute::<HashMap<&'static str, u32>>("pins");

        assert_eq!(val.as_ref().err(), None);
        let val = val.unwrap();

        assert_eq!(val["pwm"], 12);
        assert_eq!(val["a"], 29);
        assert_eq!(val["b"], 5);

        let val = PMR.components.unwrap()[2].get_attribute::<HashMap<&'static str, u8>>("pins2");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(val.err().unwrap(), AttributeError::ParseNumError);
        Ok(())
    }
}
