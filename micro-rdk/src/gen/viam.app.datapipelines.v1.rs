// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataPipeline {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// The associated Viam organization ID.
    #[prost(string, tag="2")]
    pub organization_id: ::prost::alloc::string::String,
    /// A unique identifier at the org level.
    #[prost(string, tag="3")]
    pub name: ::prost::alloc::string::String,
    /// A MongoDB aggregation pipeline as a list of BSON documents, where
    /// each document is one stage in the pipeline.
    #[prost(bytes="vec", repeated, tag="4")]
    pub mql_binary: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    /// A cron expression representing the expected execution schedule in UTC (note this also
    /// defines the input time window; an hourly schedule would process 1 hour of data at a time).
    #[prost(string, tag="5")]
    pub schedule: ::prost::alloc::string::String,
    /// Whether or not the pipeline is enabled.
    #[prost(bool, tag="6")]
    pub enabled: bool,
    /// The time the pipeline was created.
    #[prost(message, optional, tag="7")]
    pub created_on: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    /// The time the pipeline was last updated.
    #[prost(message, optional, tag="8")]
    pub updated_at: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    /// The type of data source for the pipeline. If not specified, default is standard data storage.
    #[prost(enumeration="super::super::data::v1::TabularDataSourceType", optional, tag="9")]
    pub data_source_type: ::core::option::Option<i32>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDataPipelineRequest {
    /// The ID of the data pipeline to retrieve.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDataPipelineResponse {
    #[prost(message, optional, tag="1")]
    pub data_pipeline: ::core::option::Option<DataPipeline>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListDataPipelinesRequest {
    /// The associated Viam organization ID.
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListDataPipelinesResponse {
    #[prost(message, repeated, tag="1")]
    pub data_pipelines: ::prost::alloc::vec::Vec<DataPipeline>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateDataPipelineRequest {
    /// The associated Viam organization ID.
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    /// A unique identifier at the org level.
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    /// A MongoDB aggregation pipeline as a list of BSON documents, where
    /// each document is one stage in the pipeline.
    #[prost(bytes="vec", repeated, tag="3")]
    pub mql_binary: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    /// A cron expression representing the expected execution schedule in UTC (note this also
    /// defines the input time window; an hourly schedule would process 1 hour of data at a time).
    #[prost(string, tag="4")]
    pub schedule: ::prost::alloc::string::String,
    /// When true, pipeline runs will be scheduled for the organization's past data.
    #[prost(bool, optional, tag="5")]
    pub enable_backfill: ::core::option::Option<bool>,
    /// The type of data source for the pipeline. If not specified, default is standard data storage.
    #[prost(enumeration="super::super::data::v1::TabularDataSourceType", optional, tag="6")]
    pub data_source_type: ::core::option::Option<i32>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateDataPipelineResponse {
    /// The ID of the newly created data pipeline.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RenameDataPipelineRequest {
    /// The ID of the data pipeline to rename.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// A unique identifier at the organization level.
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RenameDataPipelineResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteDataPipelineRequest {
    /// The ID of the data pipeline to delete.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteDataPipelineResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EnableDataPipelineRequest {
    /// The ID of the data pipeline to enable.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EnableDataPipelineResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DisableDataPipelineRequest {
    /// The ID of the data pipeline to disable.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DisableDataPipelineResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListDataPipelineRunsRequest {
    /// The ID of the data pipeline to list runs for.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// pagination fields
    #[prost(uint32, tag="2")]
    pub page_size: u32,
    #[prost(string, tag="3")]
    pub page_token: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListDataPipelineRunsResponse {
    /// The ID of the data pipeline the runs are for.
    #[prost(string, tag="1")]
    pub pipeline_id: ::prost::alloc::string::String,
    /// The runs that were run.
    #[prost(message, repeated, tag="2")]
    pub runs: ::prost::alloc::vec::Vec<DataPipelineRun>,
    /// A token to retrieve the next page of results.
    #[prost(string, tag="3")]
    pub next_page_token: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataPipelineRun {
    /// The ID of the run.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// The time the run started.
    #[prost(message, optional, tag="2")]
    pub start_time: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    /// The time the run ended.
    #[prost(message, optional, tag="3")]
    pub end_time: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    /// The start time of the data that was processed in the run.
    #[prost(message, optional, tag="4")]
    pub data_start_time: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    /// The end time of the data that was processed in the run.
    #[prost(message, optional, tag="5")]
    pub data_end_time: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    /// The status of the run.
    #[prost(enumeration="DataPipelineRunStatus", tag="6")]
    pub status: i32,
    /// The error message if the run failed.
    #[prost(string, tag="7")]
    pub error_message: ::prost::alloc::string::String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DataPipelineRunStatus {
    Unspecified = 0,
    Scheduled = 1,
    Started = 2,
    Completed = 3,
    Failed = 4,
}
impl DataPipelineRunStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            DataPipelineRunStatus::Unspecified => "DATA_PIPELINE_RUN_STATUS_UNSPECIFIED",
            DataPipelineRunStatus::Scheduled => "DATA_PIPELINE_RUN_STATUS_SCHEDULED",
            DataPipelineRunStatus::Started => "DATA_PIPELINE_RUN_STATUS_STARTED",
            DataPipelineRunStatus::Completed => "DATA_PIPELINE_RUN_STATUS_COMPLETED",
            DataPipelineRunStatus::Failed => "DATA_PIPELINE_RUN_STATUS_FAILED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "DATA_PIPELINE_RUN_STATUS_UNSPECIFIED" => Some(Self::Unspecified),
            "DATA_PIPELINE_RUN_STATUS_SCHEDULED" => Some(Self::Scheduled),
            "DATA_PIPELINE_RUN_STATUS_STARTED" => Some(Self::Started),
            "DATA_PIPELINE_RUN_STATUS_COMPLETED" => Some(Self::Completed),
            "DATA_PIPELINE_RUN_STATUS_FAILED" => Some(Self::Failed),
            _ => None,
        }
    }
}
// @@protoc_insertion_point(module)
