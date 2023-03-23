// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitTrainingJobRequest {
    #[prost(message, optional, tag="1")]
    pub filter: ::core::option::Option<super::super::data::v1::Filter>,
    #[prost(string, tag="2")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub model_name: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub model_version: ::prost::alloc::string::String,
    #[prost(enumeration="ModelType", tag="5")]
    pub model_type: i32,
    #[prost(string, repeated, tag="6")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitTrainingJobResponse {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetTrainingJobRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetTrainingJobResponse {
    #[prost(message, optional, tag="1")]
    pub metadata: ::core::option::Option<TrainingJobMetadata>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListTrainingJobsRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(enumeration="TrainingStatus", tag="2")]
    pub status: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListTrainingJobsResponse {
    #[prost(message, repeated, tag="1")]
    pub jobs: ::prost::alloc::vec::Vec<TrainingJobMetadata>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TrainingJobMetadata {
    #[prost(message, optional, tag="1")]
    pub request: ::core::option::Option<SubmitTrainingJobRequest>,
    #[prost(enumeration="TrainingStatus", tag="2")]
    pub status: i32,
    #[prost(message, optional, tag="3")]
    pub created_on: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="4")]
    pub last_modified: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(string, tag="5")]
    pub synced_model_id: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub user_email: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelTrainingJobRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelTrainingJobResponse {
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ModelType {
    Unspecified = 0,
    SingleLabelClassification = 1,
    MultiLabelClassification = 2,
}
impl ModelType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ModelType::Unspecified => "MODEL_TYPE_UNSPECIFIED",
            ModelType::SingleLabelClassification => "MODEL_TYPE_SINGLE_LABEL_CLASSIFICATION",
            ModelType::MultiLabelClassification => "MODEL_TYPE_MULTI_LABEL_CLASSIFICATION",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TrainingStatus {
    Unspecified = 0,
    Pending = 1,
    InProgress = 2,
    Completed = 3,
    Failed = 4,
    Canceled = 5,
}
impl TrainingStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TrainingStatus::Unspecified => "TRAINING_STATUS_UNSPECIFIED",
            TrainingStatus::Pending => "TRAINING_STATUS_PENDING",
            TrainingStatus::InProgress => "TRAINING_STATUS_IN_PROGRESS",
            TrainingStatus::Completed => "TRAINING_STATUS_COMPLETED",
            TrainingStatus::Failed => "TRAINING_STATUS_FAILED",
            TrainingStatus::Canceled => "TRAINING_STATUS_CANCELED",
        }
    }
}
// @@protoc_insertion_point(module)
