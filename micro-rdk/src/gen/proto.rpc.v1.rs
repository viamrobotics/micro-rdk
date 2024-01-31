// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Credentials {
    /// type is the type of credentials being used.
    #[prost(string, tag="1")]
    pub r#type: ::prost::alloc::string::String,
    /// payload is an opaque string used that are of the given type above.
    #[prost(string, tag="2")]
    pub payload: ::prost::alloc::string::String,
}
/// An AuthenticateRequest contains the credentials used to authenticate.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AuthenticateRequest {
    #[prost(string, tag="1")]
    pub entity: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub credentials: ::core::option::Option<Credentials>,
}
/// An AuthenticateResponse is returned after successful authentication.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AuthenticateResponse {
    /// access_token is a JWT where only the expiration should be deemed
    /// important.
    ///
    /// Future(erd): maybe a refresh_token
    #[prost(string, tag="1")]
    pub access_token: ::prost::alloc::string::String,
}
/// An AuthenticateToRequest contains the entity to authenticate to.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AuthenticateToRequest {
    #[prost(string, tag="1")]
    pub entity: ::prost::alloc::string::String,
}
/// An AuthenticateResponse is returned after successful authentication.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AuthenticateToResponse {
    /// access_token is a JWT where only the expiration should be deemed
    /// important.
    ///
    /// Future(erd): maybe a refresh_token
    #[prost(string, tag="1")]
    pub access_token: ::prost::alloc::string::String,
}
// @@protoc_insertion_point(module)
