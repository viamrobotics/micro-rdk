// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShellRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub data_in: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShellResponse {
    #[prost(string, tag="1")]
    pub data_out: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub data_err: ::prost::alloc::string::String,
    #[prost(bool, tag="3")]
    pub eof: bool,
}
/// FileData contains partial (sometimes complete) information about a File.
/// When transmitting FileData with CopyFilesToMachine and CopyFilesFromMachine,
/// it MUST initially contain its name, size, and is_dir. Depending on whether
/// preservation is in use, the mod_time and mode fields may be initially set
/// as well. On all transmissions, data and eof must be set. Because files are
/// sent one-by-one, it is currently permitted to exclude the initially set fields.
/// If this ever changes, a new scheme should be used for identifying files (like a number)
/// in order to reduce data transmission while allowing out-of-order transfers.
/// eof must be true and its own message once no more data is to be sent for this file.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileData {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(int64, tag="2")]
    pub size: i64,
    #[prost(bool, tag="3")]
    pub is_dir: bool,
    #[prost(bytes="vec", tag="4")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(bool, tag="5")]
    pub eof: bool,
    /// Note(erd): maybe support access time in the future if needed
    #[prost(message, optional, tag="6")]
    pub mod_time: ::core::option::Option<super::super::super::super::google::protobuf::Timestamp>,
    #[prost(uint32, optional, tag="7")]
    pub mode: ::core::option::Option<u32>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CopyFilesToMachineRequestMetadata {
    /// name is the service name.
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// source_type is the type of files that will be transmitted in this request stream.
    #[prost(enumeration="CopyFilesSourceType", tag="2")]
    pub source_type: i32,
    /// destination is where the files should be placed. The receiver can choose to
    /// reasonably modify this destination based on its implementation semantics.
    #[prost(string, tag="3")]
    pub destination: ::prost::alloc::string::String,
    /// preserve indicates the the receiver should use the metadata in the file to reflect
    /// the same state in its filesystem as applicable.
    #[prost(bool, tag="4")]
    pub preserve: bool,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CopyFilesToMachineRequest {
    #[prost(oneof="copy_files_to_machine_request::Request", tags="1, 2")]
    pub request: ::core::option::Option<copy_files_to_machine_request::Request>,
}
/// Nested message and enum types in `CopyFilesToMachineRequest`.
pub mod copy_files_to_machine_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Request {
        /// metadata is sent first and only once.
        #[prost(message, tag="1")]
        Metadata(super::CopyFilesToMachineRequestMetadata),
        /// file_data is sent only after metadata. All data MUST be sent
        /// in order per-file.
        #[prost(message, tag="2")]
        FileData(super::FileData),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CopyFilesToMachineResponse {
    /// value does not matter here but responses must be sent after every
    /// file has been received.
    #[prost(bool, tag="1")]
    pub ack_last_file: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CopyFilesFromMachineRequestMetadata {
    /// name is the service name.
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// paths are the paths to copy from and send back over the wire.
    #[prost(string, repeated, tag="2")]
    pub paths: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    /// allow_recursion indicates if directories should be recursed into. If
    /// a directory is encountered and this is false, an error MUST occur.
    #[prost(bool, tag="3")]
    pub allow_recursion: bool,
    /// preserve indicates the the receiver should provide the metadata in the file
    /// to reflect the same state in the sender's filesystem as applicable.
    #[prost(bool, tag="4")]
    pub preserve: bool,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CopyFilesFromMachineRequest {
    #[prost(oneof="copy_files_from_machine_request::Request", tags="1, 2")]
    pub request: ::core::option::Option<copy_files_from_machine_request::Request>,
}
/// Nested message and enum types in `CopyFilesFromMachineRequest`.
pub mod copy_files_from_machine_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Request {
        /// metadata is sent first and only once.
        #[prost(message, tag="1")]
        Metadata(super::CopyFilesFromMachineRequestMetadata),
        /// ack_last_file is sent only after metadata and after each file has been received.
        /// The value does not matter.
        #[prost(bool, tag="2")]
        AckLastFile(bool),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CopyFilesFromMachineResponseMetadata {
    /// source_type is the type of files that will be transmitted in this response stream.
    #[prost(enumeration="CopyFilesSourceType", tag="1")]
    pub source_type: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CopyFilesFromMachineResponse {
    #[prost(oneof="copy_files_from_machine_response::Response", tags="1, 2")]
    pub response: ::core::option::Option<copy_files_from_machine_response::Response>,
}
/// Nested message and enum types in `CopyFilesFromMachineResponse`.
pub mod copy_files_from_machine_response {
    #[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Response {
        /// metadata is sent first and only once.
        #[prost(message, tag="1")]
        Metadata(super::CopyFilesFromMachineResponseMetadata),
        /// file_data is sent only after metadata. All data MUST be sent
        /// in order per-file.
        #[prost(message, tag="2")]
        FileData(super::FileData),
    }
}
/// CopyFilesSourceType indicates what will be copied. It's important
/// to disambiguate the single directory case from the multiple files
/// case in order to indicate that the user's intent is to copy a directory
/// into a single location which may result in a new top-level directory versus
/// the cause of multiples files that always go into the existing target destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum CopyFilesSourceType {
    Unspecified = 0,
    SingleFile = 1,
    SingleDirectory = 2,
    MultipleFiles = 3,
}
impl CopyFilesSourceType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            CopyFilesSourceType::Unspecified => "COPY_FILES_SOURCE_TYPE_UNSPECIFIED",
            CopyFilesSourceType::SingleFile => "COPY_FILES_SOURCE_TYPE_SINGLE_FILE",
            CopyFilesSourceType::SingleDirectory => "COPY_FILES_SOURCE_TYPE_SINGLE_DIRECTORY",
            CopyFilesSourceType::MultipleFiles => "COPY_FILES_SOURCE_TYPE_MULTIPLE_FILES",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "COPY_FILES_SOURCE_TYPE_UNSPECIFIED" => Some(Self::Unspecified),
            "COPY_FILES_SOURCE_TYPE_SINGLE_FILE" => Some(Self::SingleFile),
            "COPY_FILES_SOURCE_TYPE_SINGLE_DIRECTORY" => Some(Self::SingleDirectory),
            "COPY_FILES_SOURCE_TYPE_MULTIPLE_FILES" => Some(Self::MultipleFiles),
            _ => None,
        }
    }
}
// @@protoc_insertion_point(module)
