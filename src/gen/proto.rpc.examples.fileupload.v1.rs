// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadFileRequest {
    #[prost(oneof="upload_file_request::Data", tags="1, 2")]
    pub data: ::core::option::Option<upload_file_request::Data>,
}
/// Nested message and enum types in `UploadFileRequest`.
pub mod upload_file_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Data {
        #[prost(string, tag="1")]
        Name(::prost::alloc::string::String),
        #[prost(bytes, tag="2")]
        ChunkData(::prost::alloc::vec::Vec<u8>),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadFileResponse {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(int64, tag="2")]
    pub size: i64,
}
// @@protoc_insertion_point(module)
