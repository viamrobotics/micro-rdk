// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
    #[prost(int64, tag="2")]
    pub skip: i64,
    #[prost(int64, tag="3")]
    pub limit: i64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Filter {
    #[prost(string, tag="1")]
    pub component_name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub component_type: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub component_model: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub method: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="5")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="6")]
    pub robot_name: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub robot_id: ::prost::alloc::string::String,
    #[prost(string, tag="8")]
    pub part_name: ::prost::alloc::string::String,
    #[prost(string, tag="9")]
    pub part_id: ::prost::alloc::string::String,
    #[prost(string, tag="10")]
    pub location_id: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="11")]
    pub org_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="12")]
    pub mime_type: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(message, optional, tag="13")]
    pub interval: ::core::option::Option<CaptureInterval>,
}
/// CaptureMetadata contains information on the settings used for the data capture
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CaptureMetadata {
    #[prost(string, tag="1")]
    pub org_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub location_id: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub robot_name: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub robot_id: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub part_name: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub part_id: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub component_type: ::prost::alloc::string::String,
    #[prost(string, tag="8")]
    pub component_model: ::prost::alloc::string::String,
    #[prost(string, tag="9")]
    pub component_name: ::prost::alloc::string::String,
    #[prost(string, tag="10")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(map="string, message", tag="11")]
    pub method_parameters: ::std::collections::HashMap<::prost::alloc::string::String, ::prost_types::Any>,
    #[prost(string, repeated, tag="12")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="13")]
    pub mime_type: ::prost::alloc::string::String,
}
/// CaptureInterval describes the start and end time of the capture in this file
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CaptureInterval {
    #[prost(message, optional, tag="1")]
    pub start: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="2")]
    pub end: ::core::option::Option<::prost_types::Timestamp>,
}
/// TabularDataByFilterRequest requests tabular data based on filter values
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub data_request: ::core::option::Option<DataRequest>,
    #[prost(bool, tag="2")]
    pub count_only: bool,
}
/// TabularDataByFilterResponse provides the data and metadata of tabular data
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularDataByFilterResponse {
    #[prost(message, repeated, tag="1")]
    pub metadata: ::prost::alloc::vec::Vec<CaptureMetadata>,
    #[prost(message, repeated, tag="2")]
    pub data: ::prost::alloc::vec::Vec<TabularData>,
    #[prost(int64, tag="3")]
    pub count: i64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularData {
    #[prost(message, optional, tag="1")]
    pub data: ::core::option::Option<::prost_types::Struct>,
    #[prost(int32, tag="2")]
    pub metadata_index: i32,
    #[prost(message, optional, tag="3")]
    pub time_requested: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="4")]
    pub time_received: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryData {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub uri: ::prost::alloc::string::String,
    #[prost(bytes="vec", tag="3")]
    pub binary: ::prost::alloc::vec::Vec<u8>,
    #[prost(int32, tag="4")]
    pub metadata_index: i32,
    #[prost(message, optional, tag="5")]
    pub time_requested: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="6")]
    pub time_received: ::core::option::Option<::prost_types::Timestamp>,
}
/// BinaryDataByFilterRequest requests the data and metadata of binary (image + file) data when by filter
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub data_request: ::core::option::Option<DataRequest>,
    #[prost(bool, tag="2")]
    pub include_binary: bool,
    #[prost(bool, tag="3")]
    pub count_only: bool,
}
/// BinaryDataByFilterResponse provides the data and metadata of binary (image + file) data when a filter is provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryDataByFilterResponse {
    #[prost(message, repeated, tag="1")]
    pub metadata: ::prost::alloc::vec::Vec<CaptureMetadata>,
    #[prost(message, repeated, tag="2")]
    pub data: ::prost::alloc::vec::Vec<BinaryData>,
    #[prost(int64, tag="3")]
    pub count: i64,
}
/// BinaryDataByFilterRequest requests the data and metadata of binary (image + file) data by file ids
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryDataByIDsRequest {
    #[prost(string, repeated, tag="1")]
    pub file_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    /// by default
    #[prost(bool, tag="2")]
    pub include_binary: bool,
}
/// BinaryDataByIDsResponse provides the data and metadata of binary (image + file) data when a filter is provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryDataByIDsResponse {
    #[prost(message, repeated, tag="1")]
    pub metadata: ::prost::alloc::vec::Vec<CaptureMetadata>,
    #[prost(message, repeated, tag="2")]
    pub data: ::prost::alloc::vec::Vec<BinaryData>,
    #[prost(int64, tag="3")]
    pub count: i64,
}
// @@protoc_insertion_point(module)
