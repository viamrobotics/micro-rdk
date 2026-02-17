// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVideoRequest {
    /// Name of the video source
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Start time for the video retrieval
    #[prost(message, optional, tag="2")]
    pub start_timestamp: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    /// End time for the video retrieval
    #[prost(message, optional, tag="3")]
    pub end_timestamp: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    /// Codec for the video retrieval (e.g., "h264", "h265")
    #[prost(string, tag="4")]
    pub video_codec: ::prost::alloc::string::String,
    /// Container format for the video retrieval (e.g., "mp4", "fmp4")
    #[prost(string, tag="5")]
    pub video_container: ::prost::alloc::string::String,
    /// To match a request to its responses
    #[prost(string, tag="6")]
    pub request_id: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVideoResponse {
    /// Video data chunk
    #[prost(bytes="vec", tag="1")]
    pub video_data: ::prost::alloc::vec::Vec<u8>,
    /// Container format (e.g., "mp4", "fmp4")
    #[prost(string, tag="2")]
    pub video_container: ::prost::alloc::string::String,
    /// Request ID to match this response to its request
    #[prost(string, tag="3")]
    pub request_id: ::prost::alloc::string::String,
}
// @@protoc_insertion_point(module)
