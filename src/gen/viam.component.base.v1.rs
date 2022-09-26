// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveStraightRequest {
    /// Name of a base
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Desired travel distance in millimeters
    #[prost(int64, tag="2")]
    pub distance_mm: i64,
    /// Desired travel velocity in millimeters/second
    #[prost(double, tag="3")]
    pub mm_per_sec: f64,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveStraightResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpinRequest {
    /// Name of a base
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Desired angle
    #[prost(double, tag="2")]
    pub angle_deg: f64,
    /// Desired angular velocity
    #[prost(double, tag="3")]
    pub degs_per_sec: f64,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpinResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopRequest {
    /// Name of a base
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
pub struct SetPowerRequest {
    /// Name of a base
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Desired linear power percentage as -1 -> 1
    #[prost(message, optional, tag="2")]
    pub linear: ::core::option::Option<super::super::super::common::v1::Vector3>,
    /// Desired angular power percentage % as -1 -> 1
    #[prost(message, optional, tag="3")]
    pub angular: ::core::option::Option<super::super::super::common::v1::Vector3>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetPowerResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetVelocityRequest {
    /// Name of a base
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Desired linear velocity in mm per second
    #[prost(message, optional, tag="2")]
    pub linear: ::core::option::Option<super::super::super::common::v1::Vector3>,
    /// Desired angular velocity in degrees per second
    #[prost(message, optional, tag="3")]
    pub angular: ::core::option::Option<super::super::super::common::v1::Vector3>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetVelocityResponse {
}
// @@protoc_insertion_point(module)
