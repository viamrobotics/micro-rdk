// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetInferenceRequest {
    /// The model framework and model type are inferred from the ML model registry item;
    /// For valid model types (classification, detections) we will return the formatted
    /// labels or annotations from the associated inference outputs.
    #[prost(string, tag="1")]
    pub registry_item_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub registry_item_version: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub binary_id: ::core::option::Option<super::super::data::v1::BinaryId>,
    #[prost(string, tag="4")]
    pub organization_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetInferenceResponse {
    #[prost(message, optional, tag="1")]
    pub output_tensors: ::core::option::Option<super::super::super::service::mlmodel::v1::FlatTensors>,
    #[prost(message, optional, tag="2")]
    pub annotations: ::core::option::Option<super::super::data::v1::Annotations>,
}
// @@protoc_insertion_point(module)
