// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncResponse {
}
// @@protoc_insertion_point(module)
