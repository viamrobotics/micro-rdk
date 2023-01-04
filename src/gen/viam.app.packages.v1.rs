// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileInfo {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub size: u64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PackageInfo {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub version: ::prost::alloc::string::String,
    #[prost(enumeration="PackageType", tag="4")]
    pub r#type: i32,
    #[prost(message, repeated, tag="5")]
    pub files: ::prost::alloc::vec::Vec<FileInfo>,
    #[prost(message, optional, tag="6")]
    pub metadata: ::core::option::Option<::prost_types::Struct>,
    #[prost(message, optional, tag="7")]
    pub created_on: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreatePackageRequest {
    #[prost(oneof="create_package_request::Package", tags="1, 2")]
    pub package: ::core::option::Option<create_package_request::Package>,
}
/// Nested message and enum types in `CreatePackageRequest`.
pub mod create_package_request {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Package {
        #[prost(message, tag="1")]
        Info(super::PackageInfo),
        /// .tar.gz file
        #[prost(bytes, tag="2")]
        Contents(::prost::alloc::vec::Vec<u8>),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreatePackageResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeletePackageRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="3")]
    pub versions: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeletePackageResponse {
    /// Number of versions deleted
    #[prost(int64, tag="1")]
    pub deleted_count: i64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Package {
    #[prost(message, optional, tag="1")]
    pub info: ::core::option::Option<PackageInfo>,
    #[prost(string, tag="2")]
    pub uri: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub created_on: ::core::option::Option<::prost_types::Timestamp>,
}
/// InternalPackage is stored in the packages database and represents our interval view of the uploaded package
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InternalPackage {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub version: ::prost::alloc::string::String,
    #[prost(enumeration="PackageType", tag="4")]
    pub r#type: i32,
    #[prost(message, repeated, tag="5")]
    pub files: ::prost::alloc::vec::Vec<FileInfo>,
    #[prost(message, optional, tag="6")]
    pub metadata: ::core::option::Option<::prost_types::Struct>,
    #[prost(string, tag="7")]
    pub blob_path: ::prost::alloc::string::String,
    #[prost(message, optional, tag="8")]
    pub created_on: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPackageRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub version: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPackageResponse {
    #[prost(message, optional, tag="1")]
    pub package: ::core::option::Option<Package>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListPackagesRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
    #[prost(string, optional, tag="2")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag="3")]
    pub version: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(enumeration="PackageType", optional, tag="4")]
    pub r#type: ::core::option::Option<i32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListPackagesResponse {
    #[prost(message, repeated, tag="1")]
    pub packages: ::prost::alloc::vec::Vec<Package>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PackageType {
    Unspecified = 0,
    Archive = 1,
    MlModel = 2,
}
impl PackageType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            PackageType::Unspecified => "PACKAGE_TYPE_UNSPECIFIED",
            PackageType::Archive => "PACKAGE_TYPE_ARCHIVE",
            PackageType::MlModel => "PACKAGE_TYPE_ML_MODEL",
        }
    }
}
// @@protoc_insertion_point(module)
