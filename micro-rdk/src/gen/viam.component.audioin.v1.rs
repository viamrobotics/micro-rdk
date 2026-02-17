// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAudioRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Desired duration of audio stream
    /// If not set or set to 0, the stream is infinite
    #[prost(float, tag="2")]
    pub duration_seconds: f32,
    /// Requested audio codec for the response (e.g., "mp3", "pcm16")
    #[prost(string, tag="3")]
    pub codec: ::prost::alloc::string::String,
    /// To match a request to it's responses
    #[prost(string, tag="4")]
    pub request_id: ::prost::alloc::string::String,
    /// Timestamp of the previous audio chunk, in nanoseconds, used for resuming and continuity.
    #[prost(int64, tag="5")]
    pub previous_timestamp_nanoseconds: i64,
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAudioResponse {
    #[prost(message, optional, tag="1")]
    pub audio: ::core::option::Option<AudioChunk>,
    #[prost(string, tag="2")]
    pub request_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AudioChunk {
    /// Audio data for this chunk, encoded according to the requested codec.
    #[prost(bytes="vec", tag="1")]
    pub audio_data: ::prost::alloc::vec::Vec<u8>,
    /// Info about the audio stream for this chunk
    #[prost(message, optional, tag="2")]
    pub audio_info: ::core::option::Option<super::super::super::common::v1::AudioInfo>,
    #[prost(int64, tag="3")]
    pub start_timestamp_nanoseconds: i64,
    #[prost(int64, tag="4")]
    pub end_timestamp_nanoseconds: i64,
    /// Sequential chunk number
    #[prost(int32, tag="5")]
    pub sequence: i32,
}
// @@protoc_insertion_point(module)
