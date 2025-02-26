// @generated
/// ListStreamsRequest requests all streams registered.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListStreamsRequest {
}
/// A ListStreamsResponse details streams registered.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListStreamsResponse {
    #[prost(string, repeated, tag="1")]
    pub names: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// A AddStreamRequest requests the given stream be added to the connection.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddStreamRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
/// AddStreamResponse is returned after a successful AddStreamRequest.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddStreamResponse {
}
/// A RemoveStreamRequest requests the given stream be removed from the connection.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveStreamRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
/// RemoveStreamResponse is returned after a successful RemoveStreamRequest.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveStreamResponse {
}
/// Resolution details the width and height of a stream.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Resolution {
    #[prost(int32, tag="1")]
    pub width: i32,
    #[prost(int32, tag="2")]
    pub height: i32,
}
/// GetStreamOptionsRequest requests the options for a particular stream.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStreamOptionsRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
/// GetStreamOptionsResponse details the options for a particular stream.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStreamOptionsResponse {
    #[prost(message, repeated, tag="1")]
    pub resolutions: ::prost::alloc::vec::Vec<Resolution>,
}
/// SetStreamOptionsRequest sets the options for a particular stream.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetStreamOptionsRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub resolution: ::core::option::Option<Resolution>,
}
/// SetStreamOptionsResponse is returned after a successful SetStreamOptionsRequest.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetStreamOptionsResponse {
}
// @@protoc_insertion_point(module)
