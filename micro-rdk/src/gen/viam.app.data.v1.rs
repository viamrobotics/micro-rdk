// @generated
/// DataRequest encapsulates the filter for the data, a limit on the maximum results returned,
/// a last string associated with the last returned document, and the sorting order by time.
/// last is returned in the responses TabularDataByFilterResponse and BinaryDataByFilterResponse
/// from the API calls TabularDataByFilter and BinaryDataByFilter, respectively.
/// We can then use the last string from the previous API calls in the subsequent request
/// to get the next set of ordered documents.
#[allow(clippy::derive_partial_eq_without_eq)]
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
/// Filter defines the fields over which we can filter data using a logic AND.
/// For example, if component_type and robot_id are specified, only data from that `robot_id` of
/// type `component_type` is returned. However, we logical OR over the specified tags and bounding
/// box labels, such that if component_type, robot_id, tagA, tagB are specified,
/// we return data from that `robot_id` of type `component_type` with `tagA` or `tagB`.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Filter {
    #[prost(string, tag="1")]
    pub component_name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub component_type: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub method: ::prost::alloc::string::String,
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
    pub organization_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="12")]
    pub mime_type: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(message, optional, tag="13")]
    pub interval: ::core::option::Option<CaptureInterval>,
    #[prost(message, optional, tag="14")]
    pub tags_filter: ::core::option::Option<TagsFilter>,
    /// bbox_labels are used to match documents with the specified bounding box labels (using logical OR).
    #[prost(string, repeated, tag="15")]
    pub bbox_labels: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="16")]
    pub dataset_id: ::prost::alloc::string::String,
}
/// TagsFilter defines the type of filtering and, if applicable, over which tags to perform a logical OR.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TagsFilter {
    #[prost(enumeration="TagsFilterType", tag="1")]
    pub r#type: i32,
    /// Tags are used to match documents if `type` is UNSPECIFIED or MATCH_BY_OR.
    #[prost(string, repeated, tag="2")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// CaptureMetadata contains information on the settings used for the data capture.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CaptureMetadata {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
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
    #[prost(string, tag="9")]
    pub component_name: ::prost::alloc::string::String,
    #[prost(string, tag="10")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(map="string, message", tag="11")]
    pub method_parameters: ::std::collections::HashMap<::prost::alloc::string::String, super::super::super::super::google::protobuf::Any>,
    #[prost(string, repeated, tag="12")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="13")]
    pub mime_type: ::prost::alloc::string::String,
}
/// CaptureInterval describes the start and end time of the capture in this file.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CaptureInterval {
    #[prost(message, optional, tag="1")]
    pub start: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    #[prost(message, optional, tag="2")]
    pub end: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
}
/// TabularDataByFilterRequest requests tabular data based on filter values.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub data_request: ::core::option::Option<DataRequest>,
    #[prost(bool, tag="2")]
    pub count_only: bool,
    #[prost(bool, tag="3")]
    pub include_internal_data: bool,
}
/// TabularDataByFilterResponse provides the data and metadata of tabular data.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularDataByFilterResponse {
    #[prost(message, repeated, tag="1")]
    pub metadata: ::prost::alloc::vec::Vec<CaptureMetadata>,
    #[prost(message, repeated, tag="2")]
    pub data: ::prost::alloc::vec::Vec<TabularData>,
    #[prost(uint64, tag="3")]
    pub count: u64,
    #[prost(string, tag="4")]
    pub last: ::prost::alloc::string::String,
    #[prost(uint64, tag="5")]
    pub total_size_bytes: u64,
}
/// TabularData contains data and metadata associated with tabular data.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularData {
    #[prost(message, optional, tag="1")]
    pub data: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
    #[prost(uint32, tag="2")]
    pub metadata_index: u32,
    #[prost(message, optional, tag="3")]
    pub time_requested: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    #[prost(message, optional, tag="4")]
    pub time_received: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
}
/// TabularDataBySQLRequest requests tabular data using a SQL query.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularDataBySqlRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    /// sql_query accepts any valid SQL SELECT statement. Tabular data is held in a database
    /// called "sensorData" and a table called readings, so queries should select from "readings"
    /// or "sensorData.readings".
    #[prost(string, tag="2")]
    pub sql_query: ::prost::alloc::string::String,
}
/// TabularDataBySQLResponse provides unified tabular data and metadata, queried with SQL.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularDataBySqlResponse {
    #[prost(bytes="vec", repeated, tag="2")]
    pub raw_data: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
/// TabularDataSource specifies the data source for user queries to execute on.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularDataSource {
    #[prost(enumeration="TabularDataSourceType", tag="1")]
    pub r#type: i32,
    /// pipeline_id is the ID of the pipeline to query. Required when using
    /// TABULAR_DATA_SOURCE_TYPE_PIPELINE_SINK.
    #[prost(string, optional, tag="2")]
    pub pipeline_id: ::core::option::Option<::prost::alloc::string::String>,
}
/// TabularDataByMQLRequest requests tabular data using an MQL query.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularDataByMqlRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    /// mql_binary accepts a MongoDB aggregation pipeline as a list of BSON documents, where each
    /// document is one stage in the pipeline. The pipeline is run on the "sensorData.readings"
    /// namespace, which holds the Viam organization's tabular data.
    #[prost(bytes="vec", repeated, tag="3")]
    pub mql_binary: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    /// Deprecated, please use TABULAR_DATA_SOURCE_TYPE_HOT_STORAGE instead.
    #[prost(bool, optional, tag="4")]
    pub use_recent_data: ::core::option::Option<bool>,
    /// data_source is an optional field that can be used to specify the data source for the query.
    /// If not specified, the query will run on "standard" storage.
    #[prost(message, optional, tag="6")]
    pub data_source: ::core::option::Option<TabularDataSource>,
    /// query_prefix_name is an optional field that can be used to specify a saved query to run
    #[prost(string, optional, tag="7")]
    pub query_prefix_name: ::core::option::Option<::prost::alloc::string::String>,
}
/// TabularDataByMQLResponse provides unified tabular data and metadata, queried with MQL.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularDataByMqlResponse {
    #[prost(bytes="vec", repeated, tag="2")]
    pub raw_data: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
/// ExportTabularDataRequest requests tabular data from the specified data source.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportTabularDataRequest {
    #[prost(string, tag="1")]
    pub part_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub resource_name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub resource_subtype: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="5")]
    pub interval: ::core::option::Option<CaptureInterval>,
    #[prost(message, optional, tag="6")]
    pub additional_parameters: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
/// ExportTabularDataResponse provides unified tabular data and metadata for a single data point from the specified data source.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportTabularDataResponse {
    #[prost(string, tag="1")]
    pub part_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub resource_name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub resource_subtype: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="5")]
    pub time_captured: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    #[prost(string, tag="6")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub location_id: ::prost::alloc::string::String,
    #[prost(string, tag="8")]
    pub robot_name: ::prost::alloc::string::String,
    #[prost(string, tag="9")]
    pub robot_id: ::prost::alloc::string::String,
    #[prost(string, tag="10")]
    pub part_name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="11")]
    pub method_parameters: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
    #[prost(string, repeated, tag="12")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(message, optional, tag="13")]
    pub payload: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
/// GetLatestTabularDataRequest requests the most recent tabular data captured from the specified data source.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLatestTabularDataRequest {
    #[prost(string, tag="1")]
    pub part_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub resource_name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub resource_subtype: ::prost::alloc::string::String,
    #[prost(message, optional, tag="5")]
    pub additional_parameters: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
/// GetLatestTabularDataResponse provides the data, time synced, and time captured of the most recent tabular data captured
/// from the requested data source, as long as it was synced within the last year.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLatestTabularDataResponse {
    #[prost(message, optional, tag="1")]
    pub time_captured: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    #[prost(message, optional, tag="2")]
    pub time_synced: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    #[prost(message, optional, tag="3")]
    pub payload: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
/// BinaryData contains data and metadata associated with binary data.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryData {
    #[prost(bytes="vec", tag="1")]
    pub binary: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag="2")]
    pub metadata: ::core::option::Option<BinaryMetadata>,
}
/// BinaryDataByFilterRequest requests the data and metadata of binary (image + file) data when a filter is provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub data_request: ::core::option::Option<DataRequest>,
    #[prost(bool, tag="2")]
    pub include_binary: bool,
    #[prost(bool, tag="3")]
    pub count_only: bool,
    #[prost(bool, tag="4")]
    pub include_internal_data: bool,
}
/// BinaryDataByFilterResponse provides the data and metadata of binary (image + file) data when a filter is provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryDataByFilterResponse {
    #[prost(message, repeated, tag="1")]
    pub data: ::prost::alloc::vec::Vec<BinaryData>,
    #[prost(uint64, tag="2")]
    pub count: u64,
    #[prost(string, tag="3")]
    pub last: ::prost::alloc::string::String,
    #[prost(uint64, tag="4")]
    pub total_size_bytes: u64,
}
/// BinaryID is the unique identifier for a file that one can request to be retrieved or modified.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryId {
    #[prost(string, tag="1")]
    pub file_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub location_id: ::prost::alloc::string::String,
}
/// BinaryDataByFilterRequest requests the data and metadata of binary (image + file) data by binary ids.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryDataByIDsRequest {
    #[prost(bool, tag="2")]
    pub include_binary: bool,
    #[deprecated]
    #[prost(message, repeated, tag="3")]
    pub binary_ids: ::prost::alloc::vec::Vec<BinaryId>,
    #[prost(string, repeated, tag="4")]
    pub binary_data_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// BinaryDataByIDsResponse provides the data and metadata of binary (image + file) data when a filter is provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryDataByIDsResponse {
    #[prost(message, repeated, tag="1")]
    pub data: ::prost::alloc::vec::Vec<BinaryData>,
    #[prost(uint64, tag="2")]
    pub count: u64,
}
/// BoundingBox represents a labeled bounding box on an image.
/// x and y values are normalized ratios between 0 and 1.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BoundingBox {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub label: ::prost::alloc::string::String,
    #[prost(double, tag="3")]
    pub x_min_normalized: f64,
    #[prost(double, tag="4")]
    pub y_min_normalized: f64,
    #[prost(double, tag="5")]
    pub x_max_normalized: f64,
    #[prost(double, tag="6")]
    pub y_max_normalized: f64,
    /// confidence is an optional range from 0 - 1
    #[prost(double, optional, tag="7")]
    pub confidence: ::core::option::Option<f64>,
}
/// Classification represents a confidence score with a label.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Classification {
    #[prost(string, tag="3")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="1")]
    pub label: ::prost::alloc::string::String,
    /// confidence is an optional range from 0 - 1
    #[prost(double, optional, tag="2")]
    pub confidence: ::core::option::Option<f64>,
}
/// Annotations are data annotations used for machine learning.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Annotations {
    #[prost(message, repeated, tag="1")]
    pub bboxes: ::prost::alloc::vec::Vec<BoundingBox>,
    #[prost(message, repeated, tag="2")]
    pub classifications: ::prost::alloc::vec::Vec<Classification>,
}
/// BinaryMetadata is the metadata associated with binary data.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryMetadata {
    #[deprecated]
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub capture_metadata: ::core::option::Option<CaptureMetadata>,
    #[prost(message, optional, tag="3")]
    pub time_requested: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    #[prost(message, optional, tag="4")]
    pub time_received: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    #[prost(string, tag="5")]
    pub file_name: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub file_ext: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub uri: ::prost::alloc::string::String,
    #[prost(message, optional, tag="8")]
    pub annotations: ::core::option::Option<Annotations>,
    #[prost(string, repeated, tag="9")]
    pub dataset_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="10")]
    pub binary_data_id: ::prost::alloc::string::String,
}
/// DeleteTabularDataRequest deletes the data from the organization that is older than `delete_older_than_days`
/// in UTC time. For example, if delete_older_than_days=1 and the request is made at 1AM EST on March 11
/// (March 11 5AM UTC), this deletes all data captured through March 10 11:59:59PM UTC.
/// If the request is at 10PM EST on March 11 (March 12 2AM UTC), this deletes all data captured
/// through March 11 11:59:59PM UTC.
/// If delete_older_than_days is 0, all existing data is deleted.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteTabularDataRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(uint32, tag="2")]
    pub delete_older_than_days: u32,
}
/// DeleteBinaryDataResponse returns the number of tabular datapoints deleted.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteTabularDataResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
}
/// DeleteBinaryDataByFilterRequest deletes the data and metadata of binary data when a filter is provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteBinaryDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
    #[prost(bool, tag="2")]
    pub include_internal_data: bool,
}
/// DeleteBinaryDataByFilterResponse returns the number of binary files deleted when a filter is provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteBinaryDataByFilterResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
}
/// DeleteBinaryDataByIDsRequest deletes the data and metadata of binary data when binary ids are provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteBinaryDataByIDsRequest {
    #[deprecated]
    #[prost(message, repeated, tag="2")]
    pub binary_ids: ::prost::alloc::vec::Vec<BinaryId>,
    #[prost(string, repeated, tag="3")]
    pub binary_data_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// DeleteBinaryDataByIDsResponse returns the number of binary files deleted when binary ids are provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteBinaryDataByIDsResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
}
/// AddTagsToBinaryDataByIDsRequest requests adding all specified tags to each of the files when binary ids are provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddTagsToBinaryDataByIDsRequest {
    #[deprecated]
    #[prost(message, repeated, tag="3")]
    pub binary_ids: ::prost::alloc::vec::Vec<BinaryId>,
    #[prost(string, repeated, tag="4")]
    pub binary_data_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="2")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddTagsToBinaryDataByIDsResponse {
}
/// AddTagsToBinaryDataByFilterRequest requests adding all specified tags to each of the files when a filter is provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddTagsToBinaryDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
    #[prost(string, repeated, tag="2")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddTagsToBinaryDataByFilterResponse {
}
/// RemoveTagsFromBinaryDataByIDsRequest requests removing the given tags value from each file when binary ids are provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveTagsFromBinaryDataByIDsRequest {
    #[deprecated]
    #[prost(message, repeated, tag="3")]
    pub binary_ids: ::prost::alloc::vec::Vec<BinaryId>,
    #[prost(string, repeated, tag="4")]
    pub binary_data_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="2")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// RemoveTagsFromBinaryDataByIDsResponse returns the number of binary files which had tags removed
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveTagsFromBinaryDataByIDsResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
}
/// RemoveTagsFromBinaryDataByFilterRequest requests removing the given tags value from each file when a filter is provided.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveTagsFromBinaryDataByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
    #[prost(string, repeated, tag="2")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// RemoveTagsFromBinaryDataByFilterResponse returns the number of binary files which had tags removed.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveTagsFromBinaryDataByFilterResponse {
    #[prost(uint64, tag="1")]
    pub deleted_count: u64,
}
/// TagsByFilterRequest requests the unique tags from data based on given filter.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TagsByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
}
/// TagsByFilterResponse returns the unique tags from data based on given filter.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TagsByFilterResponse {
    #[prost(string, repeated, tag="1")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// AddBoundingBoxToImageByIDRequest specifies the binary ID to which a bounding box
/// with the associated label and position in normalized coordinates will be added.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddBoundingBoxToImageByIdRequest {
    #[deprecated]
    #[prost(message, optional, tag="7")]
    pub binary_id: ::core::option::Option<BinaryId>,
    #[prost(string, tag="8")]
    pub binary_data_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub label: ::prost::alloc::string::String,
    #[prost(double, tag="3")]
    pub x_min_normalized: f64,
    #[prost(double, tag="4")]
    pub y_min_normalized: f64,
    #[prost(double, tag="5")]
    pub x_max_normalized: f64,
    #[prost(double, tag="6")]
    pub y_max_normalized: f64,
    #[prost(double, optional, tag="9")]
    pub confidence: ::core::option::Option<f64>,
}
/// AddBoundingBoxToImageByIDResponse returns the bounding box ID of the successfully added bounding box.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddBoundingBoxToImageByIdResponse {
    #[prost(string, tag="1")]
    pub bbox_id: ::prost::alloc::string::String,
}
/// RemoveBoundingBoxFromImageByIDRequest removes the bounding box with specified bounding box ID for the file represented by the binary ID.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveBoundingBoxFromImageByIdRequest {
    #[deprecated]
    #[prost(message, optional, tag="3")]
    pub binary_id: ::core::option::Option<BinaryId>,
    #[prost(string, tag="4")]
    pub binary_data_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub bbox_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveBoundingBoxFromImageByIdResponse {
}
/// UpdateBoundingBoxRequest updates the bounding box with specified bounding box ID for the file represented by the binary ID.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateBoundingBoxRequest {
    #[deprecated]
    #[prost(message, optional, tag="1")]
    pub binary_id: ::core::option::Option<BinaryId>,
    #[prost(string, tag="8")]
    pub binary_data_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub bbox_id: ::prost::alloc::string::String,
    #[prost(string, optional, tag="3")]
    pub label: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(double, optional, tag="4")]
    pub x_min_normalized: ::core::option::Option<f64>,
    #[prost(double, optional, tag="5")]
    pub y_min_normalized: ::core::option::Option<f64>,
    #[prost(double, optional, tag="6")]
    pub x_max_normalized: ::core::option::Option<f64>,
    #[prost(double, optional, tag="7")]
    pub y_max_normalized: ::core::option::Option<f64>,
    #[prost(double, optional, tag="9")]
    pub confidence: ::core::option::Option<f64>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateBoundingBoxResponse {
}
/// BoundingBoxLabelsByFilterRequest requests all the labels of the bounding boxes from files from a given filter.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BoundingBoxLabelsByFilterRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<Filter>,
}
/// BoundingBoxLabelsByFilterRequest returns all the labels of the bounding boxes from files from a given filter.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BoundingBoxLabelsByFilterResponse {
    #[prost(string, repeated, tag="1")]
    pub labels: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// ConfigureDatabaseUserRequest accepts a Viam organization ID and a password for the database user
/// being configured. Viam uses gRPC over TLS, so the entire request will be encrypted while in
/// flight, including the password.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfigureDatabaseUserRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub password: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfigureDatabaseUserResponse {
}
/// GetDatabaseConnectionRequest requests the database connection hostname.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDatabaseConnectionRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
}
/// GetDatabaseConnectionResponse returns the database connection hostname endpoint. It also returns
/// a URI that can be used to connect to the database instance through MongoDB clients, as well as
/// information on whether the Viam organization has a database user configured.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDatabaseConnectionResponse {
    #[prost(string, tag="1")]
    pub hostname: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub mongodb_uri: ::prost::alloc::string::String,
    #[prost(bool, tag="3")]
    pub has_database_user: bool,
}
/// AddBinaryDataToDatasetByIDsRequest adds the binary data with the given binary IDs to a dataset with dataset_id.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddBinaryDataToDatasetByIDsRequest {
    #[deprecated]
    #[prost(message, repeated, tag="1")]
    pub binary_ids: ::prost::alloc::vec::Vec<BinaryId>,
    #[prost(string, repeated, tag="3")]
    pub binary_data_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="2")]
    pub dataset_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddBinaryDataToDatasetByIDsResponse {
}
/// RemoveBinaryDataFromDatasetByIDsRequest removes the specified binary IDs from a dataset with dataset_id.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveBinaryDataFromDatasetByIDsRequest {
    #[deprecated]
    #[prost(message, repeated, tag="1")]
    pub binary_ids: ::prost::alloc::vec::Vec<BinaryId>,
    #[prost(string, repeated, tag="3")]
    pub binary_data_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="2")]
    pub dataset_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveBinaryDataFromDatasetByIDsResponse {
}
/// CreateIndexRequest starts a custom index build
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateIndexRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(enumeration="IndexableCollection", tag="2")]
    pub collection_type: i32,
    #[prost(string, optional, tag="3")]
    pub pipeline_name: ::core::option::Option<::prost::alloc::string::String>,
    /// index_spec accepts a MongoDB index specification defined in JSON format
    #[prost(bytes="vec", repeated, tag="4")]
    pub index_spec: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateIndexResponse {
}
/// DeleteIndexRequest drops the specified custom index from a collection
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteIndexRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(enumeration="IndexableCollection", tag="2")]
    pub collection_type: i32,
    #[prost(string, optional, tag="3")]
    pub pipeline_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, tag="4")]
    pub index_name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteIndexResponse {
}
/// ListIndexesRequest returns all the indexes for a given collection
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListIndexesRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(enumeration="IndexableCollection", tag="2")]
    pub collection_type: i32,
    #[prost(string, optional, tag="3")]
    pub pipeline_name: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListIndexesResponse {
    #[prost(message, repeated, tag="1")]
    pub indexes: ::prost::alloc::vec::Vec<Index>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Index {
    #[prost(enumeration="IndexableCollection", tag="1")]
    pub collection_type: i32,
    #[prost(string, optional, tag="2")]
    pub pipeline_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, tag="3")]
    pub index_name: ::prost::alloc::string::String,
    /// index_spec defines a MongoDB index in JSON format
    #[prost(bytes="vec", repeated, tag="4")]
    pub index_spec: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    #[prost(enumeration="IndexCreator", tag="5")]
    pub created_by: i32,
}
/// CreateSavedQueryRequest saves a mql query.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateSavedQueryRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(bytes="vec", repeated, tag="3")]
    pub mql_binary: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateSavedQueryResponse {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Query {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub name: ::prost::alloc::string::String,
    #[prost(bytes="vec", repeated, tag="4")]
    pub mql_binary: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    #[prost(message, optional, tag="5")]
    pub created_on: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    #[prost(message, optional, tag="6")]
    pub updated_at: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
}
/// DeleteSavedQuery deletes a saved query based on the given id.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteSavedQueryRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteSavedQueryResponse {
}
/// GetSavedQuery retrieves a saved query by id.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSavedQueryRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSavedQueryResponse {
    #[prost(message, optional, tag="1")]
    pub saved_query: ::core::option::Option<Query>,
}
/// UpdateSavedQuery updates the saved query with the given id.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateSavedQueryRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(bytes="vec", repeated, tag="3")]
    pub mql_binary: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateSavedQueryResponse {
}
/// ListSavedQueries lists saved queries for a given organization.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListSavedQueriesRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(int64, tag="2")]
    pub limit: i64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListSavedQueriesResponse {
    #[prost(message, repeated, tag="1")]
    pub queries: ::prost::alloc::vec::Vec<Query>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateBinaryDataSignedUrlRequest {
    /// The binary data ID of the file to create a signed URL for.
    #[prost(string, tag="1")]
    pub binary_data_id: ::prost::alloc::string::String,
    /// Expiration time in minutes. Defaults to 15 minutes if not specified.
    /// Maximum allowed is 10080 minutes (7 days).
    #[prost(uint32, optional, tag="2")]
    pub expiration_minutes: ::core::option::Option<u32>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateBinaryDataSignedUrlResponse {
    /// The signed URL for the binary data file.
    #[prost(string, tag="1")]
    pub signed_url: ::prost::alloc::string::String,
    /// Expiration time of the signed URL token.
    #[prost(message, optional, tag="2")]
    pub expires_at: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
}
/// Order specifies the order in which data is returned.
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
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ORDER_UNSPECIFIED" => Some(Self::Unspecified),
            "ORDER_DESCENDING" => Some(Self::Descending),
            "ORDER_ASCENDING" => Some(Self::Ascending),
            _ => None,
        }
    }
}
/// TagsFilterType specifies how data can be filtered based on tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TagsFilterType {
    Unspecified = 0,
    /// TAGS_FILTER_TYPE_MATCH_BY_OR specifies documents matched (using logical OR) on the tags field in the TagsFilter.
    MatchByOr = 1,
    /// TAGS_FILTER_TYPE_TAGGED specifies that all tagged documents should be returned.
    Tagged = 2,
    /// TAGS_FILTER_TYPE_UNTAGGED specifes that all untagged documents should be returned.
    Untagged = 3,
}
impl TagsFilterType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TagsFilterType::Unspecified => "TAGS_FILTER_TYPE_UNSPECIFIED",
            TagsFilterType::MatchByOr => "TAGS_FILTER_TYPE_MATCH_BY_OR",
            TagsFilterType::Tagged => "TAGS_FILTER_TYPE_TAGGED",
            TagsFilterType::Untagged => "TAGS_FILTER_TYPE_UNTAGGED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "TAGS_FILTER_TYPE_UNSPECIFIED" => Some(Self::Unspecified),
            "TAGS_FILTER_TYPE_MATCH_BY_OR" => Some(Self::MatchByOr),
            "TAGS_FILTER_TYPE_TAGGED" => Some(Self::Tagged),
            "TAGS_FILTER_TYPE_UNTAGGED" => Some(Self::Untagged),
            _ => None,
        }
    }
}
/// TabularDataSourceType specifies the data source type for TabularDataByMQL queries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TabularDataSourceType {
    Unspecified = 0,
    /// TABULAR_DATA_SOURCE_TYPE_STANDARD indicates reading from standard storage. This is
    /// the default option and available for all data synced to Viam.
    Standard = 1,
    /// TABULAR_DATA_SOURCE_TYPE_HOT_STORAGE indicates reading from hot storage. This is a
    /// premium feature requiring opting in specific data sources.
    /// See docs at <https://docs.viam.com/data-ai/capture-data/advanced/advanced-data-capture-sync/#capture-to-the-hot-data-store>
    HotStorage = 2,
    /// TABULAR_DATA_SOURCE_TYPE_PIPELINE_SINK indicates reading the output data of
    /// a data pipeline. When using this, a pipeline ID needs to be specified.
    PipelineSink = 3,
}
impl TabularDataSourceType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TabularDataSourceType::Unspecified => "TABULAR_DATA_SOURCE_TYPE_UNSPECIFIED",
            TabularDataSourceType::Standard => "TABULAR_DATA_SOURCE_TYPE_STANDARD",
            TabularDataSourceType::HotStorage => "TABULAR_DATA_SOURCE_TYPE_HOT_STORAGE",
            TabularDataSourceType::PipelineSink => "TABULAR_DATA_SOURCE_TYPE_PIPELINE_SINK",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "TABULAR_DATA_SOURCE_TYPE_UNSPECIFIED" => Some(Self::Unspecified),
            "TABULAR_DATA_SOURCE_TYPE_STANDARD" => Some(Self::Standard),
            "TABULAR_DATA_SOURCE_TYPE_HOT_STORAGE" => Some(Self::HotStorage),
            "TABULAR_DATA_SOURCE_TYPE_PIPELINE_SINK" => Some(Self::PipelineSink),
            _ => None,
        }
    }
}
/// IndexableCollection specifies the types of collections available for custom indexes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum IndexableCollection {
    Unspecified = 0,
    HotStore = 1,
    PipelineSink = 2,
}
impl IndexableCollection {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            IndexableCollection::Unspecified => "INDEXABLE_COLLECTION_UNSPECIFIED",
            IndexableCollection::HotStore => "INDEXABLE_COLLECTION_HOT_STORE",
            IndexableCollection::PipelineSink => "INDEXABLE_COLLECTION_PIPELINE_SINK",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "INDEXABLE_COLLECTION_UNSPECIFIED" => Some(Self::Unspecified),
            "INDEXABLE_COLLECTION_HOT_STORE" => Some(Self::HotStore),
            "INDEXABLE_COLLECTION_PIPELINE_SINK" => Some(Self::PipelineSink),
            _ => None,
        }
    }
}
/// IndexCreator specifies the entity that originally created the index
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum IndexCreator {
    Unspecified = 0,
    Viam = 1,
    Customer = 2,
}
impl IndexCreator {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            IndexCreator::Unspecified => "INDEX_CREATOR_UNSPECIFIED",
            IndexCreator::Viam => "INDEX_CREATOR_VIAM",
            IndexCreator::Customer => "INDEX_CREATOR_CUSTOMER",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "INDEX_CREATOR_UNSPECIFIED" => Some(Self::Unspecified),
            "INDEX_CREATOR_VIAM" => Some(Self::Viam),
            "INDEX_CREATOR_CUSTOMER" => Some(Self::Customer),
            _ => None,
        }
    }
}
// @@protoc_insertion_point(module)
