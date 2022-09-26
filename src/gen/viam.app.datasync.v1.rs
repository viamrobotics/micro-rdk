// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SensorMetadata {
    #[prost(message, optional, tag="1")]
    pub time_requested: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="2")]
    pub time_received: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SensorData {
    #[prost(message, optional, tag="1")]
    pub metadata: ::core::option::Option<SensorMetadata>,
    #[prost(oneof="sensor_data::Data", tags="2, 3")]
    pub data: ::core::option::Option<sensor_data::Data>,
}
/// Nested message and enum types in `SensorData`.
pub mod sensor_data {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Data {
        #[prost(message, tag="2")]
        Struct(::prost_types::Struct),
        #[prost(bytes, tag="3")]
        Binary(::prost::alloc::vec::Vec<u8>),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileData {
    #[prost(bytes="vec", tag="1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadMetadata {
    #[prost(string, tag="1")]
    pub part_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub component_type: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub component_name: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub component_model: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(enumeration="DataType", tag="6")]
    pub r#type: i32,
    #[prost(string, tag="7")]
    pub file_name: ::prost::alloc::string::String,
    #[prost(map="string, message", tag="8")]
    pub method_parameters: ::std::collections::HashMap<::prost::alloc::string::String, ::prost_types::Any>,
    #[prost(string, tag="9")]
    pub file_extension: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="10")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="11")]
    pub session_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadRequest {
    #[prost(oneof="upload_request::UploadPacket", tags="1, 2, 3")]
    pub upload_packet: ::core::option::Option<upload_request::UploadPacket>,
}
/// Nested message and enum types in `UploadRequest`.
pub mod upload_request {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum UploadPacket {
        #[prost(message, tag="1")]
        Metadata(super::UploadMetadata),
        #[prost(message, tag="2")]
        SensorContents(super::SensorData),
        #[prost(message, tag="3")]
        FileContents(super::FileData),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadResponse {
    #[prost(int32, tag="1")]
    pub requests_written: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CaptureInterval {
    #[prost(message, optional, tag="1")]
    pub start: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="2")]
    pub end: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataCaptureMetadata {
    #[prost(string, tag="1")]
    pub component_type: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub component_name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub component_model: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(enumeration="DataType", tag="5")]
    pub r#type: i32,
    #[prost(map="string, message", tag="6")]
    pub method_parameters: ::std::collections::HashMap<::prost::alloc::string::String, ::prost_types::Any>,
    #[prost(string, tag="7")]
    pub file_extension: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="8")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="9")]
    pub session_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TabularCapture {
    #[prost(message, optional, tag="1")]
    pub interval: ::core::option::Option<CaptureInterval>,
    #[prost(string, tag="2")]
    pub org_id: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub robot_id: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub part_id: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub location_id: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub component_name: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub component_type: ::prost::alloc::string::String,
    #[prost(string, tag="8")]
    pub component_model: ::prost::alloc::string::String,
    #[prost(string, tag="9")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(string, tag="10")]
    pub blob_path: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="11")]
    pub column_names: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(map="string, message", tag="12")]
    pub method_parameters: ::std::collections::HashMap<::prost::alloc::string::String, ::prost_types::Any>,
    #[prost(string, tag="13")]
    pub file_id: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="14")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(int32, tag="15")]
    pub message_count: i32,
    #[prost(int64, tag="16")]
    pub file_size_bytes: i64,
    #[prost(string, tag="17")]
    pub session_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BinaryCapture {
    #[prost(message, optional, tag="1")]
    pub interval: ::core::option::Option<CaptureInterval>,
    #[prost(string, tag="2")]
    pub org_id: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub robot_id: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub part_id: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub location_id: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub component_name: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub component_type: ::prost::alloc::string::String,
    #[prost(string, tag="8")]
    pub component_model: ::prost::alloc::string::String,
    #[prost(string, tag="9")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(string, tag="10")]
    pub blob_path: ::prost::alloc::string::String,
    #[prost(map="string, message", tag="11")]
    pub method_parameters: ::std::collections::HashMap<::prost::alloc::string::String, ::prost_types::Any>,
    #[prost(string, tag="12")]
    pub file_id: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="13")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(int64, tag="14")]
    pub file_size_bytes: i64,
    #[prost(string, tag="15")]
    pub session_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UserFile {
    #[prost(message, optional, tag="1")]
    pub sync_time: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(string, tag="2")]
    pub org_id: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub robot_id: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub part_id: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub location_id: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub blob_path: ::prost::alloc::string::String,
    #[prost(map="string, message", tag="7")]
    pub method_parameters: ::std::collections::HashMap<::prost::alloc::string::String, ::prost_types::Any>,
    #[prost(string, tag="8")]
    pub file_id: ::prost::alloc::string::String,
    #[prost(int64, tag="9")]
    pub file_size_bytes: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DataType {
    Unspecified = 0,
    BinarySensor = 1,
    TabularSensor = 2,
    File = 3,
}
// @@protoc_insertion_point(module)
