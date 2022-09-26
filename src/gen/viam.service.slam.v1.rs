// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionRequest {
    /// Name of slam service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPositionResponse {
    /// Current position of the robot within the World frame.
    #[prost(message, optional, tag="1")]
    pub pose: ::core::option::Option<super::super::super::common::v1::PoseInFrame>,
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
// @@protoc_insertion_point(module)
