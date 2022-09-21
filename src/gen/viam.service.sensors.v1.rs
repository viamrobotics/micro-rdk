// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSensorsRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSensorsResponse {
    #[prost(message, repeated, tag="1")]
    pub sensor_names: ::prost::alloc::vec::Vec<super::super::super::common::v1::ResourceName>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetReadingsRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, repeated, tag="2")]
    pub sensor_names: ::prost::alloc::vec::Vec<super::super::super::common::v1::ResourceName>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Readings {
    #[prost(message, optional, tag="1")]
    pub name: ::core::option::Option<super::super::super::common::v1::ResourceName>,
    #[prost(map="string, message", tag="2")]
    pub readings: ::std::collections::HashMap<::prost::alloc::string::String, ::prost_types::Value>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetReadingsResponse {
    #[prost(message, repeated, tag="1")]
    pub readings: ::prost::alloc::vec::Vec<Readings>,
}
// @@protoc_insertion_point(module)
