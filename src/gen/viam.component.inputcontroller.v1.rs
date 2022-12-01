// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetControlsRequest {
    /// Name of an input controller
    #[prost(string, tag="1")]
    pub controller: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetControlsResponse {
    /// Returns a list of all the controls (buttons and axes) that are
    /// available to a given Input Controller
    #[prost(string, repeated, tag="1")]
    pub controls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetEventsRequest {
    /// Name of an input controller
    #[prost(string, tag="1")]
    pub controller: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetEventsResponse {
    /// Returns a list of the most recent event for each control on a given InputController. Effectively provides the current "state" of all
    /// buttons/axes on a given input controller
    #[prost(message, repeated, tag="1")]
    pub events: ::prost::alloc::vec::Vec<Event>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TriggerEventRequest {
    /// Name of an input controller
    #[prost(string, tag="1")]
    pub controller: ::prost::alloc::string::String,
    /// Digitally assert a given event
    #[prost(message, optional, tag="2")]
    pub event: ::core::option::Option<Event>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TriggerEventResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Event {
    /// Timestamp of event
    #[prost(message, optional, tag="1")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
    /// An event type (eg: ButtonPress, ButtonRelease)
    #[prost(string, tag="2")]
    pub event: ::prost::alloc::string::String,
    /// A control, can be a button (eg: ButtonSouth) or an axis (eg: AbsoluteX)
    #[prost(string, tag="3")]
    pub control: ::prost::alloc::string::String,
    /// 0 or 1 for buttons, -1.0 to +1.0 for axes
    #[prost(double, tag="4")]
    pub value: f64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamEventsRequest {
    /// Name of an input controller
    #[prost(string, tag="1")]
    pub controller: ::prost::alloc::string::String,
    /// A list of Events
    #[prost(message, repeated, tag="2")]
    pub events: ::prost::alloc::vec::Vec<stream_events_request::Events>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
/// Nested message and enum types in `StreamEventsRequest`.
pub mod stream_events_request {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Events {
        /// Name of a control (button or axis)
        #[prost(string, tag="1")]
        pub control: ::prost::alloc::string::String,
        /// Specify which event types to recieve events for
        #[prost(string, repeated, tag="2")]
        pub events: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
        /// Specify which event types to stop recieving events for
        /// This can be an empty list
        #[prost(string, repeated, tag="3")]
        pub cancelled_events: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamEventsResponse {
    /// Event for a controller
    #[prost(message, optional, tag="1")]
    pub event: ::core::option::Option<Event>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Status {
    #[prost(message, repeated, tag="1")]
    pub events: ::prost::alloc::vec::Vec<Event>,
}
// @@protoc_insertion_point(module)
