// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CppFeatures {
    /// Whether or not to treat an enum field as closed.  This option is only
    /// applicable to enum fields, and will be removed in the future.  It is
    /// consistent with the legacy behavior of using proto3 enum types for proto2
    /// fields.
    #[prost(bool, optional, tag="1")]
    pub legacy_closed_enum: ::core::option::Option<bool>,
    #[prost(enumeration="cpp_features::StringType", optional, tag="2")]
    pub string_type: ::core::option::Option<i32>,
    #[prost(bool, optional, tag="3")]
    pub enum_name_uses_string_view: ::core::option::Option<bool>,
}
/// Nested message and enum types in `CppFeatures`.
pub mod cpp_features {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum StringType {
        Unknown = 0,
        View = 1,
        Cord = 2,
        String = 3,
    }
    impl StringType {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                StringType::Unknown => "STRING_TYPE_UNKNOWN",
                StringType::View => "VIEW",
                StringType::Cord => "CORD",
                StringType::String => "STRING",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "STRING_TYPE_UNKNOWN" => Some(Self::Unknown),
                "VIEW" => Some(Self::View),
                "CORD" => Some(Self::Cord),
                "STRING" => Some(Self::String),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GoFeatures {
    /// Whether or not to generate the deprecated UnmarshalJSON method for enums.
    #[prost(bool, optional, tag="1")]
    pub legacy_unmarshal_json_enum: ::core::option::Option<bool>,
    /// One of OPEN, HYBRID or OPAQUE.
    #[prost(enumeration="go_features::ApiLevel", optional, tag="2")]
    pub api_level: ::core::option::Option<i32>,
}
/// Nested message and enum types in `GoFeatures`.
pub mod go_features {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum ApiLevel {
        /// API_LEVEL_UNSPECIFIED results in selecting the OPEN API,
        /// but needs to be a separate value to distinguish between
        /// an explicitly set api level or a missing api level.
        Unspecified = 0,
        ApiOpen = 1,
        ApiHybrid = 2,
        ApiOpaque = 3,
    }
    impl ApiLevel {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                ApiLevel::Unspecified => "API_LEVEL_UNSPECIFIED",
                ApiLevel::ApiOpen => "API_OPEN",
                ApiLevel::ApiHybrid => "API_HYBRID",
                ApiLevel::ApiOpaque => "API_OPAQUE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "API_LEVEL_UNSPECIFIED" => Some(Self::Unspecified),
                "API_OPEN" => Some(Self::ApiOpen),
                "API_HYBRID" => Some(Self::ApiHybrid),
                "API_OPAQUE" => Some(Self::ApiOpaque),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct JavaFeatures {
    /// Whether or not to treat an enum field as closed.  This option is only
    /// applicable to enum fields, and will be removed in the future.  It is
    /// consistent with the legacy behavior of using proto3 enum types for proto2
    /// fields.
    #[prost(bool, optional, tag="1")]
    pub legacy_closed_enum: ::core::option::Option<bool>,
    #[prost(enumeration="java_features::Utf8Validation", optional, tag="2")]
    pub utf8_validation: ::core::option::Option<i32>,
}
/// Nested message and enum types in `JavaFeatures`.
pub mod java_features {
    /// The UTF8 validation strategy to use.  See go/editions-utf8-validation for
    /// more information on this feature.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Utf8Validation {
        /// Invalid default, which should never be used.
        Unknown = 0,
        /// Respect the UTF8 validation behavior specified by the global
        /// utf8_validation feature.
        Default = 1,
        /// Verifies UTF8 validity overriding the global utf8_validation
        /// feature. This represents the legacy java_string_check_utf8 option.
        Verify = 2,
    }
    impl Utf8Validation {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Utf8Validation::Unknown => "UTF8_VALIDATION_UNKNOWN",
                Utf8Validation::Default => "DEFAULT",
                Utf8Validation::Verify => "VERIFY",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "UTF8_VALIDATION_UNKNOWN" => Some(Self::Unknown),
                "DEFAULT" => Some(Self::Default),
                "VERIFY" => Some(Self::Verify),
                _ => None,
            }
        }
    }
}
// @@protoc_insertion_point(module)
