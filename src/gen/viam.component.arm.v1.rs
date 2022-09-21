// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetEndPositionRequest {
    /// Name of an arm
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetEndPositionResponse {
    /// Returns 6d pose of the end effector relative to the base, represented by X,Y,Z coordinates which express
    /// millimeters and theta, ox, oy, oz coordinates which express an orientation vector
    #[prost(message, optional, tag="1")]
    pub pose: ::core::option::Option<super::super::super::common::v1::Pose>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct JointPositions {
    /// A list of joint positions. Rotations values are in degrees, translational values in mm.
    /// The numbers are ordered spatially from the base toward the end effector
    /// This is used in GetJointPositionsResponse and MoveToJointPositionsRequest
    #[prost(double, repeated, tag="1")]
    pub values: ::prost::alloc::vec::Vec<f64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetJointPositionsRequest {
    /// Name of an arm
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetJointPositionsResponse {
    ///a list JointPositions
    #[prost(message, optional, tag="1")]
    pub positions: ::core::option::Option<JointPositions>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveToPositionRequest {
    /// Name of an arm
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub to: ::core::option::Option<super::super::super::common::v1::Pose>,
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
pub struct MoveToJointPositionsRequest {
    /// Name of an arm
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// A list of joint positions
    /// There should be 1 entry in the list per joint DOF, ordered spatially from the base toward the end effector
    #[prost(message, optional, tag="2")]
    pub positions: ::core::option::Option<JointPositions>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveToJointPositionsResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopRequest {
    /// Name of an arm
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
    #[prost(message, optional, tag="1")]
    pub end_position: ::core::option::Option<super::super::super::common::v1::Pose>,
    #[prost(message, optional, tag="2")]
    pub joint_positions: ::core::option::Option<JointPositions>,
    #[prost(bool, tag="3")]
    pub is_moving: bool,
}
// @@protoc_insertion_point(module)
