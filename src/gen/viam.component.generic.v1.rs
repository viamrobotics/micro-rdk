// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DoCommandRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub command: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DoCommandResponse {
    #[prost(message, optional, tag="1")]
    pub result: ::core::option::Option<::prost_types::Struct>,
}
// @@protoc_insertion_point(module)
