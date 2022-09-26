// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetPowerRequest {
    /// Name of a motor
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Percentage of motor's power, between -1 and 1
    #[prost(double, tag="2")]
    pub power_pct: f64,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetPowerResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GoForRequest {
    /// Name of a motor
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Speed of motor travel in rotations per minute
    #[prost(double, tag="2")]
    pub rpm: f64,
    /// Number of revolutions relative to motor's start position
    #[prost(double, tag="3")]
    pub revolutions: f64,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GoForResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GoToRequest {
    /// Name of a motor
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Speed of motor travel in rotations per minute
    #[prost(double, tag="2")]
    pub rpm: f64,
    /// Number of revolutions relative to motor's home home/zero
    #[prost(double, tag="3")]
    pub position_revolutions: f64,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GoToResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResetZeroPositionRequest {
    /// Name of a motor
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Motor position
    #[prost(double, tag="2")]
    pub offset: f64,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResetZeroPositionResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionRequest {
    /// Name of a motor
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionResponse {
    /// Current position of the motor relative to its home
    #[prost(double, tag="1")]
    pub position: f64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopRequest {
    /// Name of a motor
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
pub struct IsPoweredRequest {
    /// Name of a motor
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IsPoweredResponse {
    /// Returns true if the motor is on
    #[prost(bool, tag="1")]
    pub is_on: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPropertiesRequest {
    /// Name of a motor
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPropertiesResponse {
    /// Returns true if the motor supports reporting its position
    #[prost(bool, tag="1")]
    pub position_reporting: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Status {
    /// Returns true if the motor is powered
    #[prost(bool, tag="1")]
    pub is_powered: bool,
    /// Returns true if the motor has position support
    #[prost(bool, tag="2")]
    pub position_reporting: bool,
    /// Returns current position of the motor relative to its home
    #[prost(double, tag="3")]
    pub position: f64,
    /// Returns true if the motor is moving
    #[prost(bool, tag="4")]
    pub is_moving: bool,
}
// @@protoc_insertion_point(module)
