// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListUuiDsRequest {
    /// Name of the world object store service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListUuiDsResponse {
    #[prost(bytes="vec", repeated, tag="1")]
    pub uuids: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetTransformRequest {
    /// Name of the world object store service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(bytes="vec", tag="2")]
    pub uuid: ::prost::alloc::vec::Vec<u8>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetTransformResponse {
    #[prost(message, optional, tag="2")]
    pub transform: ::core::option::Option<super::super::super::common::v1::Transform>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamTransformChangesRequest {
    /// Name of the world object store service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamTransformChangesResponse {
    #[prost(enumeration="TransformChangeType", tag="1")]
    pub change_type: i32,
    #[prost(message, optional, tag="2")]
    pub transform: ::core::option::Option<super::super::super::common::v1::Transform>,
    /// The field mask of the transform that has changed, if any. For added transforms, this will be empty. For updated
    /// transforms, this will be the fields that have changed. For removed transforms, this will be the transform's UUID
    /// path.
    #[prost(message, optional, tag="3")]
    pub updated_fields: ::core::option::Option<super::super::super::super::google::protobuf::FieldMask>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TransformChangeType {
    Unspecified = 0,
    Added = 1,
    Removed = 2,
    Updated = 3,
}
impl TransformChangeType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TransformChangeType::Unspecified => "TRANSFORM_CHANGE_TYPE_UNSPECIFIED",
            TransformChangeType::Added => "TRANSFORM_CHANGE_TYPE_ADDED",
            TransformChangeType::Removed => "TRANSFORM_CHANGE_TYPE_REMOVED",
            TransformChangeType::Updated => "TRANSFORM_CHANGE_TYPE_UPDATED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "TRANSFORM_CHANGE_TYPE_UNSPECIFIED" => Some(Self::Unspecified),
            "TRANSFORM_CHANGE_TYPE_ADDED" => Some(Self::Added),
            "TRANSFORM_CHANGE_TYPE_REMOVED" => Some(Self::Removed),
            "TRANSFORM_CHANGE_TYPE_UPDATED" => Some(Self::Updated),
            _ => None,
        }
    }
}
// @@protoc_insertion_point(module)
