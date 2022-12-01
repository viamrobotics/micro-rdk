// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Result {
    #[prost(enumeration="Status", tag="1")]
    pub status: i32,
    /// message is an aggregated error message string
    #[prost(string, tag="2")]
    pub message: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
    #[prost(uint64, tag="2")]
    pub limit: u64,
    #[prost(string, tag="3")]
    pub last: ::prost::alloc::string::String,
    #[prost(enumeration="Order", tag="4")]
    pub sort_order: i32,
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
    #[prost(string, repeated, tag="10")]
    pub location_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
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
    #[prost(string, tag="4")]
    pub last: ::prost::alloc::string::String,
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
    #[prost(bytes="vec", tag="1")]
    pub binary: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag="2")]
    pub metadata: ::core::option::Option<BinaryMetadata>,
}
/// BinaryDataByFilterRequest requests the data and metadata of binary (image + file) data when a filter is provided
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
    pub data: ::prost::alloc::vec::Vec<BinaryData>,
    #[prost(uint64, tag="2")]
    pub count: u64,
    #[prost(string, tag="3")]
    pub last: ::prost::alloc::string::String,
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
    pub data: ::prost::alloc::vec::Vec<BinaryData>,
    #[prost(uint64, tag="2")]
    pub count: u64,
    #[prost(string, tag="3")]
    pub last: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryMetadata {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub capture_metadata: ::core::option::Option<CaptureMetadata>,
    #[prost(message, optional, tag="3")]
    pub time_requested: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="4")]
    pub time_received: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(string, tag="5")]
    pub file_name: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub file_ext: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub uri: ::prost::alloc::string::String,
}
/// DeleteTabularDataByFilterRequest deletes the data and metadata of tabular data when a filter is provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteTabularDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
}
/// DeleteBinaryDataByFilterResponse returns the number of tabular datapoints deleted when a filter is provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteTabularDataByFilterResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
    #[prost(message, optional, tag="2")]
    pub result: ::core::option::Option<Result>,
}
/// DeleteBinaryDataByFilterRequest deletes the data and metadata of binary data when a filter is provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteBinaryDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
}
/// DeleteBinaryDataByFilterResponse returns the number of binary files deleted when a filter is provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteBinaryDataByFilterResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
    #[prost(message, optional, tag="2")]
    pub result: ::core::option::Option<Result>,
}
/// DeleteBinaryDataByIDsRequest deletes the data and metadata of binary data when file ids are provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteBinaryDataByIDsRequest {
    #[prost(string, repeated, tag="1")]
    pub file_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// DeleteBinaryDataByIDsResponse returns the number of binary files deleted when file ids are provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteBinaryDataByIDsResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
    #[prost(message, optional, tag="2")]
    pub result: ::core::option::Option<Result>,
}
/// AddTagsToBinaryDataByFileIDsRequest requests adding all specified tags to each of the files when file IDs are provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddTagsToBinaryDataByFileIDsRequest {
    #[prost(string, repeated, tag="1")]
    pub file_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="2")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddTagsToBinaryDataByFileIDsResponse {
}
/// AddTagsToBinaryDataByFilterRequest requests adding all specified tags to each of the files when a filter is provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddTagsToBinaryDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
    #[prost(string, repeated, tag="2")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddTagsToBinaryDataByFilterResponse {
}
/// RemoveTagsFromBinaryDataByFileIDsRequest requests removing the given tags value from each file when file IDs are provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveTagsFromBinaryDataByFileIDsRequest {
    #[prost(string, repeated, tag="1")]
    pub file_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="2")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// RemoveTagsFromBinaryDataByFileIDsResponse returns the number of binary files which had tags removed
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveTagsFromBinaryDataByFileIDsResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
}
/// RemoveTagsFromBinaryDataByFilterRequest requests removing the given tags value from each file when a filter is provided
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveTagsFromBinaryDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
    #[prost(string, repeated, tag="2")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// RemoveTagsFromBinaryDataByFilterResponse returns the number of binary files which had tags removed
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveTagsFromBinaryDataByFilterResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Order {
    Unspecified = 0,
    Descending = 1,
    Ascending = 2,
}
impl Order {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Order::Unspecified => "ORDER_UNSPECIFIED",
            Order::Descending => "ORDER_DESCENDING",
            Order::Ascending => "ORDER_ASCENDING",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Status {
    Unspecified = 0,
    PartialSuccess = 1,
    Success = 2,
}
impl Status {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Status::Unspecified => "STATUS_UNSPECIFIED",
            Status::PartialSuccess => "STATUS_PARTIAL_SUCCESS",
            Status::Success => "STATUS_SUCCESS",
        }
    }
}
// @@protoc_insertion_point(module)
