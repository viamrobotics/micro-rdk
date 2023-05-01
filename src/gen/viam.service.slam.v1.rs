// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionResponse {
    /// Current position of the specified component in the SLAM Map
    #[prost(message, optional, tag="1")]
    pub pose: ::core::option::Option<super::super::super::common::v1::Pose>,
    /// This is usually the name of the camera that is in the SLAM config
    #[prost(string, tag="2")]
    pub component_reference: ::prost::alloc::string::String,
    /// Additional information in the response
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPointCloudMapRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPointCloudMapResponse {
    /// One chunk of the PointCloud.
    /// For a given GetPointCloudMap request, concatenating all
    /// GetPointCloudMapResponse.point_cloud_pcd_chunk values in the
    /// order received result in the complete pointcloud in standard PCD
    /// format.
    /// <https://pointclouds.org/documentation/tutorials/pcd_file_format.html>
    #[prost(bytes="vec", tag="1")]
    pub point_cloud_pcd_chunk: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetInternalStateRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetInternalStateResponse {
    /// Chunk of the internal state of the SLAM algorithm required to continue
    /// mapping/localization
    #[prost(bytes="vec", tag="1")]
    pub internal_state_chunk: ::prost::alloc::vec::Vec<u8>,
}
// @@protoc_insertion_point(module)
