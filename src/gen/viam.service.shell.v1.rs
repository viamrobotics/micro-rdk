// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShellRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub data_in: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShellResponse {
    #[prost(string, tag="1")]
    pub data_out: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub data_err: ::prost::alloc::string::String,
    #[prost(bool, tag="3")]
    pub eof: bool,
}
// @@protoc_insertion_point(module)
