// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileData {
    #[prost(bytes="vec", tag="1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct File {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(int64, tag="2")]
    pub size_bytes: i64,
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
    #[prost(message, repeated, tag="4")]
    pub files: ::prost::alloc::vec::Vec<File>,
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
pub struct DeleteRequest {
    #[prost(string, tag="1")]
    pub org_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub model_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeployRequest {
    #[prost(string, tag="1")]
    pub org_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub model_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Model {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(int64, tag="2")]
    pub size_bytes: i64,
    #[prost(message, repeated, tag="3")]
    pub files: ::prost::alloc::vec::Vec<File>,
    #[prost(message, optional, tag="4")]
    pub time_created: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InfoRequest {
    #[prost(string, tag="1")]
    pub org_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InfoResponse {
    #[prost(message, repeated, tag="1")]
    pub model: ::prost::alloc::vec::Vec<Model>,
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
    #[prost(message, repeated, tag="4")]
    pub files: ::prost::alloc::vec::Vec<File>,
    #[prost(int64, tag="5")]
    pub size_bytes: i64,
    #[prost(string, tag="6")]
    pub blob_path: ::prost::alloc::string::String,
    #[prost(message, optional, tag="7")]
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
impl Status {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Status::Unspecified => "UNSPECIFIED",
            Status::Fail => "FAIL",
            Status::Ok => "OK",
        }
    }
}
// @@protoc_insertion_point(module)
