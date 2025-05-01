#![allow(dead_code)]
#[cfg(feature = "data")]
use crate::common::data_collector::DataCollectorConfig;
use crate::{
    google,
    proto::{
        app::{agent::v1::DeviceAgentConfigResponse, v1::ComponentConfig},
        common,
        provisioning::v1::SetNetworkCredentialsRequest,
    },
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::{
    collections::HashMap,
    num::{ParseFloatError, ParseIntError},
};
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
    #[error(transparent)]
    ConfigApiErr(#[from] ConfigApiError),
    #[error(transparent)]
    ConfigModelErr(#[from] ConfigModelError),
}

#[derive(Error, Debug, Eq, PartialEq)]
pub enum ConfigApiError {
    #[error("invalid api string {0}")]
    ConfigApiErrorInvalidApiString(String),
    #[error("type: {0} is empty or not an alphanumeric string")]
    ConfigApiErrorResourceType(String),
    #[error("subtype: {0} is empty or not an alphanumeric string")]
    ConfigApiErrorResourceSubType(String),
    #[error("namespace: {0} is empty or not an alphanumeric string")]
    ConfigApiErrorResourceNamespace(String),
}

#[derive(Error, Debug, Eq, PartialEq)]
pub enum ConfigModelError {
    #[error("invalid model familly string {0}")]
    ConfigModelErrorInvalidModelString(String),
    #[error("model: {0} is empty or not an alphanumeric string")]
    ConfigModelErrorInvalidModel(String),
    #[error("familly: {0} is empty or not an alphanumeric string")]
    ConfigModelErrorInvalidFamilly(String),
    #[error("namespace: {0} is empty or not an alphanumeric string")]
    ConfigModelErrorInvalidNamespace(String),
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

pub struct Model {
    familly: String,
    model: String,
    namespace: String,
}

impl Model {
    pub fn new_builtin(model: String) -> Self {
        Self {
            familly: "builtin".to_owned(),
            namespace: "rdk".to_owned(),
            model,
        }
    }
    pub fn get_model(&self) -> &str {
        &self.model
    }
    pub fn get_familly(&self) -> &str {
        &self.familly
    }
    pub fn get_namespace(&self) -> &str {
        &self.namespace
    }
}
impl Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "Model({}:{}:{})",
            self.namespace, self.familly, self.model
        ))
    }
}
// this is equivalent to the regex \w-
fn is_valid_api_model_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

// for both model and api we follow the triplet constrains of RDK
impl TryFrom<&str> for Model {
    type Error = ConfigModelError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // we split the string at ':' if we get three elements that match is_valid_api_model_char we have a valid triplet
        let mut iter = value.split(":");
        let namespace = iter
            .next()
            .ok_or(ConfigModelError::ConfigModelErrorInvalidModelString(
                value.to_owned(),
            ))
            .and_then(|s| {
                (!s.is_empty() && s.chars().all(is_valid_api_model_char))
                    .then_some(s)
                    .ok_or(ConfigModelError::ConfigModelErrorInvalidNamespace(
                        s.to_owned(),
                    ))
            })?;
        let familly = iter
            .next()
            .ok_or(ConfigModelError::ConfigModelErrorInvalidFamilly(
                value.to_owned(),
            ))
            .and_then(|s| {
                (!s.is_empty() && s.chars().all(is_valid_api_model_char))
                    .then_some(s)
                    .ok_or(ConfigModelError::ConfigModelErrorInvalidFamilly(
                        s.to_owned(),
                    ))
            })?;
        let model = iter
            .next()
            .ok_or(ConfigModelError::ConfigModelErrorInvalidModel(
                value.to_owned(),
            ))
            .and_then(|s| {
                (!s.is_empty() && s.chars().all(is_valid_api_model_char))
                    .then_some(s)
                    .ok_or(ConfigModelError::ConfigModelErrorInvalidModel(s.to_owned()))
            })?;
        // if there is extra stuff after the third word we return an error
        iter.next().map_or(Ok(()), |_| {
            Err(ConfigModelError::ConfigModelErrorInvalidModelString(
                value.to_owned(),
            ))
        })?;
        Ok(Self {
            namespace: namespace.to_owned(),
            familly: familly.to_owned(),
            model: model.to_owned(),
        })
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct API {
    r#type: String,
    subtype: String,
    namespace: String,
}

impl Debug for API {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "API({}:{}:{})",
            self.namespace, self.r#type, self.subtype
        ))
    }
}

impl TryFrom<&str> for API {
    type Error = ConfigApiError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut iter = value.split(":");
        let namespace = iter
            .next()
            .ok_or(ConfigApiError::ConfigApiErrorInvalidApiString(
                value.to_owned(),
            ))
            .and_then(|s| {
                (!s.is_empty() && s.chars().all(is_valid_api_model_char))
                    .then_some(s)
                    .ok_or(ConfigApiError::ConfigApiErrorResourceNamespace(
                        s.to_owned(),
                    ))
            })?;
        let r#type = iter
            .next()
            .ok_or(ConfigApiError::ConfigApiErrorResourceType(value.to_owned()))
            .and_then(|s| {
                (!s.is_empty() && s.chars().all(is_valid_api_model_char))
                    .then_some(s)
                    .ok_or(ConfigApiError::ConfigApiErrorResourceType(s.to_owned()))
            })?;
        let subtype = iter
            .next()
            .ok_or(ConfigApiError::ConfigApiErrorResourceSubType(
                value.to_owned(),
            ))
            .and_then(|s| {
                (!s.is_empty() && s.chars().all(is_valid_api_model_char))
                    .then_some(s)
                    .ok_or(ConfigApiError::ConfigApiErrorResourceSubType(s.to_owned()))
            })?;
        iter.next().map_or(Ok(()), |_| {
            Err(ConfigApiError::ConfigApiErrorInvalidApiString(
                value.to_owned(),
            ))
        })?;
        Ok(Self {
            namespace: namespace.to_owned(),
            subtype: subtype.to_owned(),
            r#type: r#type.to_owned(),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ResourceName {
    name: String,
    api: API,
}

impl ResourceName {
    pub fn new(name: String, api: API) -> Self {
        Self { name, api }
    }
    // as opposite the RDK we only support rdk:component:xxxx api triplets (external module will not have their own namespace)
    pub fn new_builtin(name: String, subtype: String) -> Self {
        let api = API {
            namespace: "rdk".to_owned(),
            r#type: "component".to_owned(),
            subtype,
        };
        Self { api, name }
    }
    pub fn get_api(&self) -> &API {
        &self.api
    }
    pub fn get_type(&self) -> &str {
        &self.api.r#type
    }
    pub fn get_subtype(&self) -> &str {
        &self.api.subtype
    }
    pub fn get_namespace(&self) -> &str {
        &self.api.namespace
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
    // rpc GetResourceNames doesn't use the api string
    pub fn to_proto_resource_name(&self) -> common::v1::ResourceName {
        common::v1::ResourceName {
            namespace: self.api.namespace.to_string(),
            r#type: self.api.r#type.to_string(),
            subtype: self.api.subtype.to_string(),
            name: self.name.to_string(),
            local_name: self.name.to_string(),
            remote_path: vec![],
        }
    }
}

#[derive(Debug)]
pub struct DynamicComponentConfig {
    pub(crate) name: ResourceName,
    pub(crate) model: Model,
    pub attributes: Option<HashMap<String, Kind>>,
    #[cfg(feature = "data")]
    pub data_collector_configs: Vec<DataCollectorConfig>,
}

impl DynamicComponentConfig {
    pub fn get_resource_name(&self) -> &ResourceName {
        &self.name
    }
    pub fn get_model(&self) -> &Model {
        &self.model
    }
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
        let api: API = value.api.as_str().try_into()?;
        Ok(Self {
            name: ResourceName::new(value.name.clone(), api),
            model: value.model.as_str().try_into()?,
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
    #[deprecated(since = "0.5.1", note = "get_type() is deprecated use get_subtype()")]
    pub fn get_type(&self) -> &str {
        match self {
            Self::Dynamic(cfg) => cfg.get_resource_name().get_subtype(),
        }
    }
    pub fn get_subtype(&self) -> &str {
        match self {
            Self::Dynamic(cfg) => cfg.get_resource_name().get_subtype(),
        }
    }
}

pub trait Component {
    fn get_resource_name(&self) -> &ResourceName;
    fn get_model(&self) -> &Model;
    #[deprecated(
        since = "0.5.1",
        note = "get_name() is deprecated use get_resource_name().get_name()"
    )]
    fn get_name(&self) -> &str;
    #[deprecated(
        since = "0.5.1",
        note = "get_type() is deprecated use get_resource_name().get_type()"
    )]
    fn get_type(&self) -> &str;
    #[deprecated(
        since = "0.5.1",
        note = "get_subtype() is deprecated use get_resource_name().get_subtype()"
    )]
    fn get_subtype(&self) -> &str;
    #[deprecated(
        since = "0.5.1",
        note = "get_namespace() is deprecated use get_resource_name().get_namespace()"
    )]
    fn get_namespace(&self) -> &str;
    fn get_attribute<'a, T>(&'a self, key: &str) -> Result<T, AttributeError>
    where
        T: std::convert::TryFrom<&'a Kind, Error = AttributeError>;
}

impl Component for DynamicComponentConfig {
    fn get_resource_name(&self) -> &ResourceName {
        &self.name
    }
    fn get_model(&self) -> &Model {
        &self.model
    }
    fn get_name(&self) -> &str {
        self.name.get_name()
    }
    fn get_type(&self) -> &str {
        self.name.get_type()
    }
    fn get_subtype(&self) -> &str {
        self.name.get_subtype()
    }
    fn get_namespace(&self) -> &str {
        self.name.get_namespace()
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

#[derive(Debug)]
pub struct AgentConfig {
    pub network_settings: Vec<NetworkSetting>,
}

impl TryFrom<&DeviceAgentConfigResponse> for AgentConfig {
    type Error = AttributeError;
    fn try_from(value: &DeviceAgentConfigResponse) -> Result<Self, Self::Error> {
        if let Some(additional_networks) = &value.additional_networks {
            let network_settings = additional_networks
                .fields
                .iter()
                .filter_map(|(_k, v)| {
                    let local_kind: Option<Kind> =
                        v.kind.clone().and_then(|v| Kind::try_from(v).ok());
                    local_kind
                        .as_ref()
                        .and_then(|v| NetworkSetting::try_from(v).ok())
                })
                .collect::<Vec<NetworkSetting>>();
            Ok(Self { network_settings })
        } else {
            Err(AttributeError::ConversionImpossibleError)
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkSetting {
    pub(crate) ssid: String,
    pub(crate) password: String,
    pub(crate) priority: i32,
}

impl NetworkSetting {
    pub fn new(ssid: String, password: String, priority: i32) -> Self {
        Self {
            ssid,
            password,
            priority,
        }
    }
}

impl From<SetNetworkCredentialsRequest> for NetworkSetting {
    fn from(value: SetNetworkCredentialsRequest) -> Self {
        Self {
            ssid: value.ssid,
            password: value.psk,
            priority: 0,
        }
    }
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
        let priority: i32 = value
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

use core::cmp::Ordering;
impl Ord for NetworkSetting {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.priority == other.priority {
            return self.ssid.cmp(&other.ssid);
        }
        other.priority.cmp(&self.priority)
    }
}

impl PartialOrd for NetworkSetting {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::config::{
        AttributeError, Component, ConfigModelError, DynamicComponentConfig, Kind, Model,
        ResourceName,
    };

    use super::{ConfigApiError, API};

    #[test_log::test]
    fn test_model_api() {
        let model1: Result<Model, ConfigModelError> = "rdk:builtin:sensor".try_into();
        assert!(model1.is_ok());

        let model1: Result<Model, ConfigModelError> = "rdk:builtin_b:sensor".try_into();
        assert!(model1.is_ok());

        let model1: Result<Model, ConfigModelError> = "rdk:builtin:sensor-d".try_into();
        assert!(model1.is_ok());

        let model1: Result<Model, ConfigModelError> = "rdk:builtin:movement_sensor".try_into();
        assert!(model1.is_ok());

        let model1: Result<Model, ConfigModelError> = "rdk::sensor".try_into();
        assert!(model1.is_err());
        assert!(matches!(
            model1.unwrap_err(),
            ConfigModelError::ConfigModelErrorInvalidFamilly(_)
        ));

        let model1: Result<Model, ConfigModelError> = "rdk:ok:sensor+g".try_into();
        assert!(model1.is_err());
        assert!(matches!(
            model1.unwrap_err(),
            ConfigModelError::ConfigModelErrorInvalidModel(_)
        ));

        let model1: Result<Model, ConfigModelError> = ":".try_into();
        assert!(model1.is_err());
        assert!(matches!(
            model1.unwrap_err(),
            ConfigModelError::ConfigModelErrorInvalidNamespace(_)
        ));

        let model1: Result<Model, ConfigModelError> = "aa:bb:vv:dd".try_into();
        assert!(model1.is_err());
        assert!(matches!(
            model1.unwrap_err(),
            ConfigModelError::ConfigModelErrorInvalidModelString(_)
        ));

        let api1: Result<API, ConfigApiError> = "rdk:component:acon".try_into();
        assert!(api1.is_ok());

        let api1: Result<API, ConfigApiError> = "rdk:component:acon_p".try_into();
        assert!(api1.is_ok());

        let api1: Result<API, ConfigApiError> = "rdk:component:acon-d".try_into();
        assert!(api1.is_ok());

        let api1: Result<API, ConfigApiError> = "rdk:component:acon-d:DD".try_into();
        assert!(api1.is_err());
        assert!(matches!(
            api1.unwrap_err(),
            ConfigApiError::ConfigApiErrorInvalidApiString(_)
        ));

        let api1: Result<API, ConfigApiError> = "rdk:component:acon@@".try_into();
        assert!(api1.is_err());
        assert!(matches!(
            api1.unwrap_err(),
            ConfigApiError::ConfigApiErrorResourceSubType(_)
        ));

        let api1: Result<API, ConfigApiError> = "rdk::acon@@".try_into();
        assert!(api1.is_err());
        assert!(matches!(
            api1.unwrap_err(),
            ConfigApiError::ConfigApiErrorResourceType(_)
        ));

        let api1: Result<API, ConfigApiError> = "rdk&&::".try_into();
        assert!(api1.is_err());
        assert!(matches!(
            api1.unwrap_err(),
            ConfigApiError::ConfigApiErrorResourceNamespace(_)
        ));
    }

    #[test_log::test]
    fn test_config_component() {
        let robot_config: [DynamicComponentConfig; 3] = [
            DynamicComponentConfig {
                name: ResourceName::new_builtin("board".to_owned(), "board".to_owned()),
                model: Model::new_builtin("pi".to_owned()),
                data_collector_configs: vec![],
                attributes: Some(HashMap::from([(
                    "pins".to_owned(),
                    Kind::VecValue(vec![
                        Kind::StringValue("11".to_owned()),
                        Kind::StringValue("12".to_owned()),
                        Kind::StringValue("13".to_owned()),
                    ]),
                )])),
            },
            DynamicComponentConfig {
                name: ResourceName::new_builtin("motor".to_owned(), "motor".to_owned()),
                model: Model::new_builtin("gpio".to_owned()),
                data_collector_configs: vec![],
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
            },
            DynamicComponentConfig {
                name: ResourceName::new_builtin("motor".to_owned(), "motor".to_owned()),
                model: Model::new_builtin("gpio".to_owned()),
                data_collector_configs: vec![],
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
            },
        ];

        let val = robot_config[0].get_resource_name().get_name();
        assert_eq!(val, "board");

        let val = robot_config[0].get_model().get_model();
        assert_eq!(val, "pi");

        let val = robot_config[0].get_resource_name().get_subtype();
        assert_eq!(val, "board");

        let val = robot_config[1].get_resource_name().get_name();
        assert_eq!(val, "motor");

        let val = robot_config[1].get_model().get_model();
        assert_eq!(val, "gpio");

        let val = robot_config[1].get_resource_name().get_subtype();
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
            name: "component".to_owned(),
            api: "rdk:builtin:sensor".to_owned(),
            model: "rdk:builtin:sensor".to_owned(),
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
