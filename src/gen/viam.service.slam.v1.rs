// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionResponse {
    /// Current position of the robot within the World frame.
    #[prost(message, optional, tag="1")]
    pub pose: ::core::option::Option<super::super::super::common::v1::PoseInFrame>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMapRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Requested MIME type of response (image/jpeg or image/pcd)
    #[prost(string, tag="2")]
    pub mime_type: ::prost::alloc::string::String,
    /// Optional parameter for image/jpeg mime_type, used to project point
    /// cloud into a 2D image.
    #[prost(message, optional, tag="3")]
    pub camera_position: ::core::option::Option<super::super::super::common::v1::Pose>,
    /// Optional parameter for image/jpeg mime_type, defaults to false.
    /// Tells us whether to include the robot position on the 2D image.
    #[prost(bool, tag="4")]
    pub include_robot_marker: bool,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMapResponse {
    /// Actual MIME type of response (image/jpeg or image/pcd)
    #[prost(string, tag="3")]
    pub mime_type: ::prost::alloc::string::String,
    /// Image or point cloud of mime_type.
    #[prost(oneof="get_map_response::Map", tags="1, 2")]
    pub map: ::core::option::Option<get_map_response::Map>,
}
/// Nested message and enum types in `GetMapResponse`.
pub mod get_map_response {
    /// Image or point cloud of mime_type.
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Map {
        #[prost(message, tag="1")]
        PointCloud(super::super::super::super::common::v1::PointCloudObject),
        #[prost(bytes, tag="2")]
        Image(::prost::alloc::vec::Vec<u8>),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionNewRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionNewResponse {
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
    /// pointclouds are returned as a set of bytes in the standard PCD format
    /// <https://pointclouds.org/documentation/tutorials/pcd_file_format.html>
    #[prost(bytes="vec", tag="1")]
    pub point_cloud_pcd: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetInternalStateRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetInternalStateResponse {
    /// A chunk of the internal state of the SLAM algorithm required to continue
    /// mapping/localization
    #[prost(bytes="vec", tag="1")]
    pub internal_state: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPointCloudMapStreamRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPointCloudMapStreamResponse {
    /// One chunk of the PointCloud.
    /// For a given GetPointCloudMapStream request,
    /// concatinating all
    /// GetPointCloudMapStreamResponse.point_cloud_pcd_chunk
    /// values in the order received result in
    /// the complete pointcloud in standard PCD format.
    /// <https://pointclouds.org/documentation/tutorials/pcd_file_format.html>
    #[prost(bytes="vec", tag="1")]
    pub point_cloud_pcd_chunk: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetInternalStateStreamRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetInternalStateStreamResponse {
    /// Chunk of the internal state of the SLAM algorithm required to continue
    /// mapping/localization
    #[prost(bytes="vec", tag="1")]
    pub internal_state_chunk: ::prost::alloc::vec::Vec<u8>,
}
// @@protoc_insertion_point(module)
