// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DiscoverResourcesRequest {
    /// name of the discover service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DiscoverResourcesResponse {
    /// list of ComponentConfigs that describe the components found by a discover service.
    #[prost(message, repeated, tag="1")]
    pub discoveries: ::prost::alloc::vec::Vec<super::super::super::app::v1::ComponentConfig>,
}
// @@protoc_insertion_point(module)
