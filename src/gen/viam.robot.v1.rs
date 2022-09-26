// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FrameSystemConfig {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub pose_in_parent_frame: ::core::option::Option<super::super::common::v1::PoseInFrame>,
    #[prost(bytes="vec", tag="3")]
    pub model_json: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FrameSystemConfigRequest {
    /// pose information on any additional reference frames that are needed
    /// to supplement the robot's frame system
    #[prost(message, repeated, tag="1")]
    pub supplemental_transforms: ::prost::alloc::vec::Vec<super::super::common::v1::Transform>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FrameSystemConfigResponse {
    #[prost(message, repeated, tag="1")]
    pub frame_system_configs: ::prost::alloc::vec::Vec<FrameSystemConfig>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransformPoseRequest {
    /// the original pose to transform along with the reference frame in
    /// which it was observed
    #[prost(message, optional, tag="1")]
    pub source: ::core::option::Option<super::super::common::v1::PoseInFrame>,
    /// the reference frame into which the source pose should be transformed,
    /// if unset this defaults to the "world" reference frame
    #[prost(string, tag="2")]
    pub destination: ::prost::alloc::string::String,
    /// pose information on any additional reference frames that are needed
    /// to perform the transform
    #[prost(message, repeated, tag="3")]
    pub supplemental_transforms: ::prost::alloc::vec::Vec<super::super::common::v1::Transform>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransformPoseResponse {
    #[prost(message, optional, tag="1")]
    pub pose: ::core::option::Option<super::super::common::v1::PoseInFrame>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceNamesRequest {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceNamesResponse {
    #[prost(message, repeated, tag="1")]
    pub resources: ::prost::alloc::vec::Vec<super::super::common::v1::ResourceName>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceRpcSubtype {
    #[prost(message, optional, tag="1")]
    pub subtype: ::core::option::Option<super::super::common::v1::ResourceName>,
    #[prost(string, tag="2")]
    pub proto_service: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceRpcSubtypesRequest {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceRpcSubtypesResponse {
    #[prost(message, repeated, tag="1")]
    pub resource_rpc_subtypes: ::prost::alloc::vec::Vec<ResourceRpcSubtype>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Operation {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub method: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub arguments: ::core::option::Option<::prost_types::Struct>,
    #[prost(message, optional, tag="4")]
    pub started: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOperationsRequest {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOperationsResponse {
    #[prost(message, repeated, tag="1")]
    pub operations: ::prost::alloc::vec::Vec<Operation>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelOperationRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelOperationResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BlockForOperationRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BlockForOperationResponse {
}
// Discovery

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DiscoveryQuery {
    #[prost(string, tag="1")]
    pub subtype: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub model: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Discovery {
    #[prost(message, optional, tag="1")]
    pub query: ::core::option::Option<DiscoveryQuery>,
    #[prost(message, optional, tag="2")]
    pub results: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DiscoverComponentsRequest {
    #[prost(message, repeated, tag="1")]
    pub queries: ::prost::alloc::vec::Vec<DiscoveryQuery>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DiscoverComponentsResponse {
    #[prost(message, repeated, tag="1")]
    pub discovery: ::prost::alloc::vec::Vec<Discovery>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Status {
    #[prost(message, optional, tag="1")]
    pub name: ::core::option::Option<super::super::common::v1::ResourceName>,
    #[prost(message, optional, tag="2")]
    pub status: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStatusRequest {
    #[prost(message, repeated, tag="1")]
    pub resource_names: ::prost::alloc::vec::Vec<super::super::common::v1::ResourceName>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStatusResponse {
    #[prost(message, repeated, tag="1")]
    pub status: ::prost::alloc::vec::Vec<Status>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamStatusRequest {
    #[prost(message, repeated, tag="1")]
    pub resource_names: ::prost::alloc::vec::Vec<super::super::common::v1::ResourceName>,
    /// how often to send a new status.
    #[prost(message, optional, tag="2")]
    pub every: ::core::option::Option<::prost_types::Duration>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamStatusResponse {
    #[prost(message, repeated, tag="1")]
    pub status: ::prost::alloc::vec::Vec<Status>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopExtraParameters {
    #[prost(message, optional, tag="1")]
    pub name: ::core::option::Option<super::super::common::v1::ResourceName>,
    #[prost(message, optional, tag="2")]
    pub params: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopAllRequest {
    #[prost(message, repeated, tag="99")]
    pub extra: ::prost::alloc::vec::Vec<StopExtraParameters>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopAllResponse {
}
// @@protoc_insertion_point(module)
