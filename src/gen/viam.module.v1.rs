// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddResourceRequest {
    #[prost(message, optional, tag="1")]
    pub config: ::core::option::Option<super::super::app::v1::ComponentConfig>,
    #[prost(string, repeated, tag="2")]
    pub dependencies: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddResourceResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReconfigureResourceRequest {
    #[prost(message, optional, tag="1")]
    pub config: ::core::option::Option<super::super::app::v1::ComponentConfig>,
    #[prost(string, repeated, tag="2")]
    pub dependencies: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReconfigureResourceResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveResourceRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveResourceResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HandlerDefinition {
    #[prost(message, optional, tag="1")]
    pub subtype: ::core::option::Option<super::super::robot::v1::ResourceRpcSubtype>,
    #[prost(string, repeated, tag="2")]
    pub models: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HandlerMap {
    #[prost(message, repeated, tag="1")]
    pub handlers: ::prost::alloc::vec::Vec<HandlerDefinition>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyRequest {
    #[prost(string, tag="1")]
    pub parent_address: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyResponse {
    #[prost(bool, tag="1")]
    pub ready: bool,
    #[prost(message, optional, tag="2")]
    pub handlermap: ::core::option::Option<HandlerMap>,
}
// @@protoc_insertion_point(module)
