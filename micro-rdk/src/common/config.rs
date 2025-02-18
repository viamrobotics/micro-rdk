#![allow(dead_code)]
#[cfg(feature = "data")]
use crate::common::data_collector::DataCollectorConfig;
use crate::google;
use crate::proto::{
    app::{agent::v1::DeviceAgentConfigResponse, v1::ComponentConfig},
    common::v1::ResourceName,
};

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
    #[error("{0}")]
    ValidationError(String),
}

impl TryFrom<&DeviceAgentConfigResponse> for AgentConfig {
    type Error = AttributeError;
    fn try_from(value: &DeviceAgentConfigResponse) -> Result<Self, Self::Error> {
        if let Some(ref additional_networks) = value.additional_networks {
            let network_settings = additional_networks
                .fields
                .iter()
                .filter_map(|(_k, v)| {
                    let network_kind: &Kind = &v
                        .kind
                        .clone()
                        .ok_or(AttributeError::ConversionImpossibleError)
                        .ok()
                        .as_ref()
                        .unwrap()
                        .try_into()
                        .unwrap();

                    network_kind.try_into().ok()
                })
                .collect();
            Ok(Self { network_settings })
        } else {
            Err(AttributeError::ConversionImpossibleError)
        }
    }
}

#[derive(Debug)]
pub struct AgentConfig {
    network_settings: Vec<NetworkSetting>,
}

impl TryFrom<&Kind> for NetworkSetting {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        let ssid: String = value
            .get("ssid")?
            .ok_or(AttributeError::ConversionImpossibleError)?
            .try_into()?;
        let password: String = value
            .get("psk")?
            .ok_or(AttributeError::ConversionImpossibleError)?
            .try_into()?;
        let priority: usize = value
            .get("priority")?
            .ok_or(AttributeError::ConversionImpossibleError)?
            .try_into()?;
        Ok(Self {
            ssid,
            password,
            priority,
        })
    }
}

impl std::fmt::Debug for NetworkSetting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NetworkSetting {{ ssid: {}, password: ***, priority: {} }}",
            self.ssid, self.priority
        )
    }
}

pub struct NetworkSetting {
    ssid: String,
    password: String,
    priority: usize,
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
        $(
          impl TryFrom<&Kind> for $t
          {
              type Error = AttributeError;
              fn try_from(value: &Kind) -> Result<Self, Self::Error> {
                  match value {
                      Kind::NullValue(v) => Ok(*v as $t),
                      Kind::NumberValue(v) => Ok(*v as $t),
                      Kind::BoolValue(v) => Ok(*v as $t),
                      Kind::StringValue(v) => Ok(v.parse::<$t>()?),
                      _ => Err(AttributeError::ConversionImpossibleError),
                  }
              }
          }
        )*
    }
}
primitives!(u32, i32, u8, u16, i16, i8, usize);

macro_rules! floats
{
    ( $($t:ty),* ) =>
    {
        $(
          impl TryFrom<&Kind> for $t
          {
              type Error = AttributeError;
              fn try_from(value: &Kind) -> Result<Self, Self::Error> {
                  match value {
                      Kind::NullValue(v) => Ok(*v as $t),
                      Kind::NumberValue(v) => Ok(*v as $t),
                      Kind::StringValue(v) => Ok(v.parse::<$t>()?),
                      _ => Err(AttributeError::ConversionImpossibleError),
                  }
              }
          }
        )*
    }
}

floats!(f64, f32);

impl<'b, V> TryFrom<&'b Kind> for HashMap<&'b str, V>
where
    V: std::convert::TryFrom<&'b Kind, Error = AttributeError>,
{
    type Error = AttributeError;
    fn try_from(value: &'b Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StructValue(v) => v
                .iter()
                .map(|(k, v)| (Ok((k.as_str(), v.try_into()?))))
                .collect(),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl<'a, T> TryFrom<&'a Kind> for Vec<T>
where
    T: std::convert::TryFrom<&'a Kind, Error = AttributeError>,
{
    type Error = AttributeError;
    fn try_from(value: &'a Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::VecValue(v) => v.iter().map(|v| v.try_into()).collect(),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl<'b> TryFrom<&'b Kind> for &'b str {
    type Error = AttributeError;
    fn try_from(value: &'b Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StringValue(v) => Ok(v.as_str()),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<&Kind> for String {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StringValue(v) => Ok(v.to_string()),
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
            Kind::NumberValue(v) => Ok(Kind::NumberValue(*v)),
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

#[derive(Debug, Clone)]
pub enum Kind {
    NullValue(i32),
    NumberValue(f64),
    StringValue(String),
    BoolValue(bool),
    VecValue(Vec<Kind>),
    StructValue(HashMap<String, Kind>),
}

impl Kind {
    pub fn get(&self, key: &str) -> Result<Option<&Kind>, AttributeError> {
        match self {
            Self::StructValue(v) => Ok(v.get(key)),
            _ => Err(AttributeError::KeyNotFound(key.to_string())),
        }
    }

    pub fn contains_key(&self, key: &str) -> Result<bool, AttributeError> {
        match self {
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
pub struct DynamicComponentConfig {
    pub name: String,
    pub namespace: String,
    pub r#type: String,
    pub model: String,
    pub attributes: Option<HashMap<String, Kind>>,
    #[cfg(feature = "data")]
    pub data_collector_configs: Vec<DataCollectorConfig>,
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
        #[cfg(feature = "data")]
        let data_collector_configs = if !value.service_configs.is_empty() {
            if let Some(data_service_cfg) = value
                .service_configs
                .iter()
                .find(|cfg| cfg.r#type == *"rdk:service:data_manager")
            {
                let data_service_attributes_struct =
                    &data_service_cfg.attributes.as_ref().unwrap().fields;

                let capture_methods_val =
                    data_service_attributes_struct.get(&("capture_methods".to_string()));
                match capture_methods_val {
                    Some(capture_methods_val) => {
                        if let Some(capture_methods_proto) = capture_methods_val.kind.as_ref() {
                            let capture_methods_kind: Kind = capture_methods_proto.try_into()?;
                            let capture_methods: Vec<DataCollectorConfig> =
                                (&capture_methods_kind).try_into()?;
                            capture_methods
                        } else {
                            vec![]
                        }
                    }
                    None => vec![],
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        Ok(Self {
            name: value.name.to_string(),
            namespace: value.namespace.to_string(),
            r#type: value.r#type.to_string(),
            model: value.model.to_string(),
            attributes: attrs_opt,
            #[cfg(feature = "data")]
            data_collector_configs,
        })
    }
}

#[derive(Debug)]
pub enum ConfigType<'a> {
    Dynamic(&'a DynamicComponentConfig),
}

impl<'a> ConfigType<'a> {
    pub fn get_attribute<T>(&'a self, key: &str) -> Result<T, AttributeError>
    where
        T: std::convert::TryFrom<&'a Kind, Error = AttributeError>,
    {
        match self {
            Self::Dynamic(cfg) => cfg.get_attribute::<T>(key),
        }
    }
    pub fn get_type(&self) -> &str {
        match self {
            Self::Dynamic(cfg) => cfg.get_type(),
        }
    }
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
            local_name: self.get_name().to_string(),
            remote_path: vec![],
        }
    }
    fn get_attribute<'a, T>(&'a self, key: &str) -> Result<T, AttributeError>
    where
        T: std::convert::TryFrom<&'a Kind, Error = AttributeError>;
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
        T: std::convert::TryFrom<&'a Kind, Error = AttributeError>,
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

    use crate::common::config::{AttributeError, Component, DynamicComponentConfig, Kind};

    #[test_log::test]
    fn test_config_component() {
        let robot_config: [DynamicComponentConfig; 3] = [
            DynamicComponentConfig {
                name: "board".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "board".to_owned(),
                model: "pi".to_owned(),
                attributes: Some(HashMap::from([(
                    "pins".to_owned(),
                    Kind::VecValue(vec![
                        Kind::StringValue("11".to_owned()),
                        Kind::StringValue("12".to_owned()),
                        Kind::StringValue("13".to_owned()),
                    ]),
                )])),
                ..Default::default()
            },
            DynamicComponentConfig {
                name: "motor".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([
                            ("a".to_owned(), Kind::StringValue("29".to_owned())),
                            ("b".to_owned(), Kind::StringValue("5".to_owned())),
                            ("pwm".to_owned(), Kind::StringValue("12".to_owned())),
                        ])),
                    ),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                ])),
                ..Default::default()
            },
            DynamicComponentConfig {
                name: "motor".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    ("float".to_owned(), Kind::NumberValue(10.556)),
                    ("float2".to_owned(), Kind::StringValue("10.564".to_owned())),
                    (
                        "float3".to_owned(),
                        Kind::StringValue("-1.18e+11".to_owned()),
                    ),
                    (
                        "pins".to_owned(),
                        Kind::VecValue(vec![
                            Kind::StringValue("11000".to_owned()),
                            Kind::StringValue("12".to_owned()),
                            Kind::StringValue("13".to_owned()),
                        ]),
                    ),
                    (
                        "pins2".to_owned(),
                        Kind::StructValue(HashMap::from([(
                            "a".to_owned(),
                            Kind::StringValue("29000".to_owned()),
                        )])),
                    ),
                ])),
                ..Default::default()
            },
        ];

        let val = robot_config[0].get_name();
        assert_eq!(val, "board");

        let val = robot_config[0].get_model();
        assert_eq!(val, "pi");

        let val = robot_config[0].get_type();
        assert_eq!(val, "board");

        let val = robot_config[1].get_name();
        assert_eq!(val, "motor");

        let val = robot_config[1].get_model();
        assert_eq!(val, "gpio");

        let val = robot_config[1].get_type();
        assert_eq!(val, "motor");

        let val = robot_config[1].get_attribute::<u32>("board");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(val.err().unwrap(), AttributeError::ParseNumError);

        let val = robot_config[1].get_attribute::<u32>("nope");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(
            val.err().unwrap(),
            AttributeError::KeyNotFound("nope".to_string())
        );

        let val = robot_config[0].get_attribute::<u32>("pins");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(
            val.err().unwrap(),
            AttributeError::ConversionImpossibleError
        );

        let val = robot_config[2].get_attribute::<f32>("float");

        assert_eq!(val.as_ref().err(), None);
        assert_eq!(val.ok().unwrap(), 10.556);

        let val = robot_config[2].get_attribute::<f32>("float2");

        assert_eq!(val.as_ref().err(), None);
        assert_eq!(val.ok().unwrap(), 10.564);

        let val = robot_config[2].get_attribute::<f64>("float3");

        assert_eq!(val.as_ref().err(), None);
        assert_eq!(val.ok().unwrap(), -1.18e+11);

        let val = robot_config[0].get_attribute::<u32>("pins");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(
            val.err().unwrap(),
            AttributeError::ConversionImpossibleError
        );

        let val = robot_config[0].get_attribute::<Vec<i8>>("pins");

        assert_eq!(val.ok().unwrap(), vec![11, 12, 13]);

        let val = robot_config[2].get_attribute::<Vec<i8>>("pins");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(val.err().unwrap(), AttributeError::ParseNumError);

        let val = robot_config[1].get_attribute::<HashMap<&str, u32>>("pins");

        assert_eq!(val.as_ref().err(), None);
        let val = val.unwrap();

        assert_eq!(val["pwm"], 12);
        assert_eq!(val["a"], 29);
        assert_eq!(val["b"], 5);

        let val = robot_config[2].get_attribute::<HashMap<&str, u8>>("pins2");

        assert_eq!(val.as_ref().ok(), None);
        assert_eq!(val.err().unwrap(), AttributeError::ParseNumError);
    }

    #[cfg(feature = "data")]
    #[test_log::test]
    fn test_data_collector_config_parsing() {
        use crate::proto::app::v1::{ComponentConfig, ResourceLevelServiceConfig};
        use crate::{
            common::data_collector::CollectionMethod,
            google::protobuf::{value::Kind as PKind, ListValue, Struct, Value},
        };

        let comp_config = ComponentConfig {
            service_configs: vec![
                ResourceLevelServiceConfig {
                    r#type: "rdk:service:some_service".to_string(),
                    attributes: None,
                },
                ResourceLevelServiceConfig {
                    r#type: "rdk:service:data_manager".to_string(),
                    attributes: Some(Struct {
                        fields: HashMap::from([(
                            "capture_methods".to_string(),
                            Value {
                                kind: Some(PKind::ListValue(ListValue {
                                    values: vec![
                                        Value {
                                            kind: Some(PKind::StructValue(Struct {
                                                fields: HashMap::from([
                                                    (
                                                        "method".to_string(),
                                                        Value {
                                                            kind: Some(PKind::StringValue(
                                                                "Readings".to_string(),
                                                            )),
                                                        },
                                                    ),
                                                    (
                                                        "capture_frequency_hz".to_string(),
                                                        Value {
                                                            kind: Some(PKind::NumberValue(100.0)),
                                                        },
                                                    ),
                                                ]),
                                            })),
                                        },
                                        Value {
                                            kind: Some(PKind::StructValue(Struct {
                                                fields: HashMap::from([
                                                    (
                                                        "method".to_string(),
                                                        Value {
                                                            kind: Some(PKind::StringValue(
                                                                "Readings".to_string(),
                                                            )),
                                                        },
                                                    ),
                                                    (
                                                        "capture_frequency_hz".to_string(),
                                                        Value {
                                                            kind: Some(PKind::NumberValue(200.0)),
                                                        },
                                                    ),
                                                ]),
                                            })),
                                        },
                                    ],
                                })),
                            },
                        )]),
                    }),
                },
            ],
            ..Default::default()
        };

        let comp_config_parsed = DynamicComponentConfig::try_from(&comp_config);
        assert!(comp_config_parsed.is_ok());
        let comp_config_parsed = comp_config_parsed.unwrap();
        let data_coll_cfgs = comp_config_parsed.data_collector_configs;
        assert_eq!(data_coll_cfgs.len(), 2);

        let data_coll = &data_coll_cfgs[0];
        assert_eq!(data_coll.capture_frequency_hz, 100.0);
        assert!(matches!(data_coll.method, CollectionMethod::Readings));

        let data_coll = &data_coll_cfgs[1];
        assert_eq!(data_coll.capture_frequency_hz, 200.0);
        assert!(matches!(data_coll.method, CollectionMethod::Readings));
    }
}
