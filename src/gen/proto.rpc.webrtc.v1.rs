// @generated
/// A PacketMessage is used to packetize large messages (> 64KiB) to be able to safely
/// transmit over WebRTC data channels.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PacketMessage {
    #[prost(bytes="vec", tag="1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(bool, tag="2")]
    pub eom: bool,
}
/// A Stream represents an instance of a gRPC stream between
/// a client and a server.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Stream {
    #[prost(uint64, tag="1")]
    pub id: u64,
}
/// A Request is a frame coming from a client. It is always
/// associated with a stream where the client assigns the stream
/// identifier. Servers will drop frames where the stream identifier
/// has no association (if a non-header frames are sent).
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Request {
    #[prost(message, optional, tag="1")]
    pub stream: ::core::option::Option<Stream>,
    #[prost(oneof="request::Type", tags="2, 3, 4")]
    pub r#type: ::core::option::Option<request::Type>,
}
/// Nested message and enum types in `Request`.
pub mod request {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Type {
        #[prost(message, tag="2")]
        Headers(super::RequestHeaders),
        #[prost(message, tag="3")]
        Message(super::RequestMessage),
        #[prost(bool, tag="4")]
        RstStream(bool),
    }
}
/// RequestHeaders describe the unary or streaming call to make.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RequestHeaders {
    #[prost(string, tag="1")]
    pub method: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub metadata: ::core::option::Option<Metadata>,
    #[prost(message, optional, tag="3")]
    pub timeout: ::core::option::Option<::prost_types::Duration>,
}
/// A RequestMessage contains individual gRPC messages and a potential
/// end-of-stream (EOS) marker.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RequestMessage {
    #[prost(bool, tag="1")]
    pub has_message: bool,
    #[prost(message, optional, tag="2")]
    pub packet_message: ::core::option::Option<PacketMessage>,
    #[prost(bool, tag="3")]
    pub eos: bool,
}
/// A Response is a frame coming from a server. It is always
/// associated with a stream where the client assigns the stream
/// identifier. Clients will drop frames where the stream identifier
/// has no association.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Response {
    #[prost(message, optional, tag="1")]
    pub stream: ::core::option::Option<Stream>,
    #[prost(oneof="response::Type", tags="2, 3, 4")]
    pub r#type: ::core::option::Option<response::Type>,
}
/// Nested message and enum types in `Response`.
pub mod response {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Type {
        #[prost(message, tag="2")]
        Headers(super::ResponseHeaders),
        #[prost(message, tag="3")]
        Message(super::ResponseMessage),
        #[prost(message, tag="4")]
        Trailers(super::ResponseTrailers),
    }
}
/// ResponseHeaders contain custom metadata that are sent to the client
/// before any message or trailers (unless only trailers are sent).
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResponseHeaders {
    #[prost(message, optional, tag="1")]
    pub metadata: ::core::option::Option<Metadata>,
}
/// ResponseMessage contains the data of a response to a call.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResponseMessage {
    #[prost(message, optional, tag="1")]
    pub packet_message: ::core::option::Option<PacketMessage>,
}
/// ResponseTrailers contain the status of a response and any custom metadata.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResponseTrailers {
    #[prost(message, optional, tag="1")]
    pub status: ::core::option::Option<super::super::super::super::google::rpc::Status>,
    #[prost(message, optional, tag="2")]
    pub metadata: ::core::option::Option<Metadata>,
}
/// Strings are a series of values.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Strings {
    #[prost(string, repeated, tag="1")]
    pub values: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// Metadata is for custom key values provided by a client or server
/// during a stream.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Metadata {
    #[prost(map="string, message", tag="1")]
    pub md: ::std::collections::HashMap<::prost::alloc::string::String, Strings>,
}
/// ICECandidate represents an ICE candidate.
/// From <https://github.com/pion/webrtc/blob/5f6baf73255598a7b4a7c9400bb0381acc9aa3dc/icecandidateinit.go>
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IceCandidate {
    #[prost(string, tag="1")]
    pub candidate: ::prost::alloc::string::String,
    #[prost(string, optional, tag="2")]
    pub sdp_mid: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag="3")]
    pub sdpm_line_index: ::core::option::Option<u32>,
    #[prost(string, optional, tag="4")]
    pub username_fragment: ::core::option::Option<::prost::alloc::string::String>,
}
/// CallRequest is the SDP offer that the controlling side is making.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallRequest {
    #[prost(string, tag="1")]
    pub sdp: ::prost::alloc::string::String,
    /// when disable_trickle is true, the init stage will be the only stage
    /// to be received in the response and the caller can expect the SDP
    /// to contain all ICE candidates.
    #[prost(bool, tag="2")]
    pub disable_trickle: bool,
}
/// CallResponseInitStage is the first and a one time stage that represents
/// the initial response to starting a call.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallResponseInitStage {
    #[prost(string, tag="1")]
    pub sdp: ::prost::alloc::string::String,
}
/// CallResponseUpdateStage is multiply used to trickle in ICE candidates from
/// the controlled (answering) side.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallResponseUpdateStage {
    #[prost(message, optional, tag="1")]
    pub candidate: ::core::option::Option<IceCandidate>,
}
/// CallResponse is the SDP answer that the controlled side responds with.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallResponse {
    #[prost(string, tag="1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(oneof="call_response::Stage", tags="2, 3")]
    pub stage: ::core::option::Option<call_response::Stage>,
}
/// Nested message and enum types in `CallResponse`.
pub mod call_response {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Stage {
        #[prost(message, tag="2")]
        Init(super::CallResponseInitStage),
        #[prost(message, tag="3")]
        Update(super::CallResponseUpdateStage),
    }
}
/// CallUpdateRequest updates the call with additional info to the controlled side.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallUpdateRequest {
    #[prost(string, tag="1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(oneof="call_update_request::Update", tags="2, 3, 4")]
    pub update: ::core::option::Option<call_update_request::Update>,
}
/// Nested message and enum types in `CallUpdateRequest`.
pub mod call_update_request {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Update {
        #[prost(message, tag="2")]
        Candidate(super::IceCandidate),
        #[prost(bool, tag="3")]
        Done(bool),
        #[prost(message, tag="4")]
        Error(super::super::super::super::super::google::rpc::Status),
    }
}
/// CallUpdateResponse contains nothing in response to a call update.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallUpdateResponse {
}
/// ICEServer describes an ICE server.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IceServer {
    #[prost(string, repeated, tag="1")]
    pub urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="2")]
    pub username: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub credential: ::prost::alloc::string::String,
}
/// WebRTCConfig represents parts of a WebRTC config.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WebRtcConfig {
    #[prost(message, repeated, tag="1")]
    pub additional_ice_servers: ::prost::alloc::vec::Vec<IceServer>,
    /// disable_trickle indicates if Trickle ICE should be used. Currently, both
    /// sides must both respect this setting.
    #[prost(bool, tag="2")]
    pub disable_trickle: bool,
}
/// AnswerRequestInitStage is the first and a one time stage that represents the
/// callers initial SDP request to the controlled (answerer) side.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerRequestInitStage {
    #[prost(string, tag="1")]
    pub sdp: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub optional_config: ::core::option::Option<WebRtcConfig>,
    #[prost(message, optional, tag="3")]
    pub deadline: ::core::option::Option<::prost_types::Timestamp>,
}
/// AnswerRequestUpdateStage is multiply used to trickle in ICE candidates to
/// the controlled (answerer) side.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerRequestUpdateStage {
    #[prost(message, optional, tag="1")]
    pub candidate: ::core::option::Option<IceCandidate>,
}
/// AnswerRequestDoneStage indicates the controller is done responding with candidates.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerRequestDoneStage {
}
/// AnswerRequestErrorStage indicates the exchange has failed with an error.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerRequestErrorStage {
    #[prost(message, optional, tag="1")]
    pub status: ::core::option::Option<super::super::super::super::google::rpc::Status>,
}
/// AnswerRequest is the SDP offer that the controlling side is making via the answering
/// stream.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerRequest {
    #[prost(string, tag="1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(oneof="answer_request::Stage", tags="2, 3, 4, 5")]
    pub stage: ::core::option::Option<answer_request::Stage>,
}
/// Nested message and enum types in `AnswerRequest`.
pub mod answer_request {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Stage {
        #[prost(message, tag="2")]
        Init(super::AnswerRequestInitStage),
        #[prost(message, tag="3")]
        Update(super::AnswerRequestUpdateStage),
        /// done is sent when the requester is done sending information
        #[prost(message, tag="4")]
        Done(super::AnswerRequestDoneStage),
        /// error is sent any time before done
        #[prost(message, tag="5")]
        Error(super::AnswerRequestErrorStage),
    }
}
/// AnswerResponseInitStage is the first and a one time stage that represents the
/// answerers initial SDP response to the controlling side.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerResponseInitStage {
    #[prost(string, tag="1")]
    pub sdp: ::prost::alloc::string::String,
}
/// AnswerResponseUpdateStage is multiply used to trickle in ICE candidates to
/// the controlling side.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerResponseUpdateStage {
    #[prost(message, optional, tag="1")]
    pub candidate: ::core::option::Option<IceCandidate>,
}
/// AnswerResponseDoneStage indicates the answerer is done responding with candidates.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerResponseDoneStage {
}
/// AnswerResponseErrorStage indicates the exchange has failed with an error.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerResponseErrorStage {
    #[prost(message, optional, tag="1")]
    pub status: ::core::option::Option<super::super::super::super::google::rpc::Status>,
}
/// AnswerResponse is the SDP answer that an answerer responds with.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnswerResponse {
    #[prost(string, tag="1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(oneof="answer_response::Stage", tags="2, 3, 4, 5")]
    pub stage: ::core::option::Option<answer_response::Stage>,
}
/// Nested message and enum types in `AnswerResponse`.
pub mod answer_response {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Stage {
        #[prost(message, tag="2")]
        Init(super::AnswerResponseInitStage),
        #[prost(message, tag="3")]
        Update(super::AnswerResponseUpdateStage),
        /// done is sent when the answerer is done sending information
        #[prost(message, tag="4")]
        Done(super::AnswerResponseDoneStage),
        /// error is sent any time before done
        #[prost(message, tag="5")]
        Error(super::AnswerResponseErrorStage),
    }
}
/// OptionalWebRTCConfigRequest is the request for getting an optional WebRTC config
/// to use for the peer connection.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OptionalWebRtcConfigRequest {
}
/// OptionalWebRTCConfigResponse contains the optional WebRTC config
/// to use for the peer connection.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OptionalWebRtcConfigResponse {
    #[prost(message, optional, tag="1")]
    pub config: ::core::option::Option<WebRtcConfig>,
}
// @@protoc_insertion_point(module)
