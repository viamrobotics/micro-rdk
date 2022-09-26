// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileData {
    #[prost(bytes="vec", tag="1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadMetadata {
    #[prost(string, tag="1")]
    pub org_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub model_name: ::prost::alloc::string::String,
    /// TODO: Determine the format of the associated dataset. Update when it's decided
    /// whether it should be by ID or name.
    #[prost(string, tag="3")]
    pub associated_dataset: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadRequest {
    #[prost(oneof="upload_request::UploadPacket", tags="1, 2")]
    pub upload_packet: ::core::option::Option<upload_request::UploadPacket>,
}
/// Nested message and enum types in `UploadRequest`.
pub mod upload_request {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum UploadPacket {
        #[prost(message, tag="1")]
        Metadata(super::UploadMetadata),
        #[prost(message, tag="2")]
        FileContents(super::FileData),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteMetadata {
    #[prost(string, tag="1")]
    pub org_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub model_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRequest {
    #[prost(message, optional, tag="1")]
    pub metadata: ::core::option::Option<DeleteMetadata>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeployMetadata {
    #[prost(string, tag="1")]
    pub model_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeployRequest {
    #[prost(message, optional, tag="1")]
    pub metadata: ::core::option::Option<DeployMetadata>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadResponse {
    #[prost(string, tag="1")]
    pub message: ::prost::alloc::string::String,
    #[prost(enumeration="Status", tag="2")]
    pub status: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteResponse {
    #[prost(string, tag="1")]
    pub message: ::prost::alloc::string::String,
    #[prost(enumeration="Status", tag="2")]
    pub status: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeployResponse {
    #[prost(string, tag="1")]
    pub message: ::prost::alloc::string::String,
    #[prost(enumeration="Status", tag="2")]
    pub status: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncedModel {
    #[prost(string, tag="1")]
    pub org_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub model_name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub associated_dataset: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub blob_path: ::prost::alloc::string::String,
    #[prost(message, optional, tag="5")]
    pub sync_time: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Status {
    /// buf:lint:ignore ENUM_VALUE_PREFIX
    /// buf:lint:ignore ENUM_ZERO_VALUE_SUFFIX
    Unspecified = 0,
    /// buf:lint:ignore ENUM_VALUE_PREFIX
    Fail = 1,
    /// buf:lint:ignore ENUM_VALUE_PREFIX
    Ok = 2,
}
// @@protoc_insertion_point(module)
