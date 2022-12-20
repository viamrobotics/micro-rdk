// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionResponse {
    #[prost(double, repeated, tag="1")]
    pub positions_mm: ::prost::alloc::vec::Vec<f64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveToPositionRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(double, repeated, tag="2")]
    pub positions_mm: ::prost::alloc::vec::Vec<f64>,
    #[prost(message, optional, tag="3")]
    pub world_state: ::core::option::Option<super::super::super::common::v1::WorldState>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveToPositionResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLengthsRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLengthsResponse {
    #[prost(double, repeated, tag="1")]
    pub lengths_mm: ::prost::alloc::vec::Vec<f64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopRequest {
    /// Name of a gantry
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Status {
    #[prost(double, repeated, tag="1")]
    pub positions_mm: ::prost::alloc::vec::Vec<f64>,
    #[prost(double, repeated, tag="2")]
    pub lengths_mm: ::prost::alloc::vec::Vec<f64>,
    #[prost(bool, tag="3")]
    pub is_moving: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsMovingRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsMovingResponse {
    #[prost(bool, tag="1")]
    pub is_moving: bool,
}
// @@protoc_insertion_point(module)
