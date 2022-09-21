// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub destination: ::core::option::Option<super::super::super::common::v1::PoseInFrame>,
    #[prost(message, optional, tag="3")]
    pub component_name: ::core::option::Option<super::super::super::common::v1::ResourceName>,
    #[prost(message, optional, tag="4")]
    pub world_state: ::core::option::Option<super::super::super::common::v1::WorldState>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveResponse {
    #[prost(bool, tag="1")]
    pub success: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveSingleComponentRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub destination: ::core::option::Option<super::super::super::common::v1::PoseInFrame>,
    #[prost(message, optional, tag="3")]
    pub component_name: ::core::option::Option<super::super::super::common::v1::ResourceName>,
    #[prost(message, optional, tag="4")]
    pub world_state: ::core::option::Option<super::super::super::common::v1::WorldState>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveSingleComponentResponse {
    #[prost(bool, tag="1")]
    pub success: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPoseRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// the component whose pose is being requested
    #[prost(message, optional, tag="2")]
    pub component_name: ::core::option::Option<super::super::super::common::v1::ResourceName>,
    /// the reference frame in which the component's pose
    /// should be provided, if unset this defaults
    /// to the "world" reference frame
    #[prost(string, tag="3")]
    pub destination_frame: ::prost::alloc::string::String,
    /// pose information on any additional reference frames that are needed
    /// to compute the component's pose
    #[prost(message, repeated, tag="4")]
    pub supplemental_transforms: ::prost::alloc::vec::Vec<super::super::super::common::v1::Transform>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPoseResponse {
    #[prost(message, optional, tag="1")]
    pub pose: ::core::option::Option<super::super::super::common::v1::PoseInFrame>,
}
// @@protoc_insertion_point(module)
