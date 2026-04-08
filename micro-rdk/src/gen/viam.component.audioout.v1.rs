// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlayRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(bytes="vec", tag="2")]
    pub audio_data: ::prost::alloc::vec::Vec<u8>,
    /// Info describing the audio_data
    #[prost(message, optional, tag="3")]
    pub audio_info: ::core::option::Option<super::super::super::common::v1::AudioInfo>,
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlayResponse {
}
// @@protoc_insertion_point(module)
