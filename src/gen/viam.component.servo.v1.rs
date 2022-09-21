// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveRequest {
    /// the name of the servo, as registered
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// the degrees by which to rotate the servo. Accepted values are between 0 and 180
    #[prost(uint32, tag="2")]
    pub angle_deg: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionRequest {
    /// the name of the servo, as registered
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionResponse {
    /// the degrees from neutral by which the servo is currently rotated. Values are between 0 and 180
    #[prost(uint32, tag="1")]
    pub position_deg: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopRequest {
    /// Name of a servo
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Status {
    #[prost(uint32, tag="1")]
    pub position_deg: u32,
    #[prost(bool, tag="2")]
    pub is_moving: bool,
}
// @@protoc_insertion_point(module)
