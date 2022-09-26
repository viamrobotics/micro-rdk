// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetReadingsRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetReadingsResponse {
    #[prost(map="string, message", tag="1")]
    pub readings: ::std::collections::HashMap<::prost::alloc::string::String, ::prost_types::Value>,
}
// @@protoc_insertion_point(module)
