// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TunnelRequest {
    #[prost(uint32, tag="1")]
    pub destination_port: u32,
    #[prost(bytes="vec", tag="2")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TunnelResponse {
    #[prost(bytes="vec", tag="1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FrameSystemConfig {
    /// this is an experimental API message
    #[prost(message, optional, tag="1")]
    pub frame: ::core::option::Option<super::super::common::v1::Transform>,
    #[prost(message, optional, tag="2")]
    pub kinematics: ::core::option::Option<super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FrameSystemConfigRequest {
    /// pose information on any additional reference frames that are needed
    /// to supplement the robot's frame system
    #[prost(message, repeated, tag="1")]
    pub supplemental_transforms: ::prost::alloc::vec::Vec<super::super::common::v1::Transform>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FrameSystemConfigResponse {
    #[prost(message, repeated, tag="1")]
    pub frame_system_configs: ::prost::alloc::vec::Vec<FrameSystemConfig>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransformPoseRequest {
    /// the original pose to transform along with the reference frame in
    /// which it was observed
    #[prost(message, optional, tag="1")]
    pub source: ::core::option::Option<super::super::common::v1::PoseInFrame>,
    /// the reference frame into which the source pose should be transformed,
    /// if unset this defaults to the "world" reference frame
    #[prost(string, tag="2")]
    pub destination: ::prost::alloc::string::String,
    /// pose information on any additional reference frames that are needed
    /// to perform the transform
    #[prost(message, repeated, tag="3")]
    pub supplemental_transforms: ::prost::alloc::vec::Vec<super::super::common::v1::Transform>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransformPoseResponse {
    #[prost(message, optional, tag="1")]
    pub pose: ::core::option::Option<super::super::common::v1::PoseInFrame>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransformPcdRequest {
    /// the point clouds to transform. This should be in the PCD format
    /// encoded into bytes: <https://pointclouds.org/documentation/tutorials/pcd_file_format.html>
    #[prost(bytes="vec", tag="1")]
    pub point_cloud_pcd: ::prost::alloc::vec::Vec<u8>,
    /// the reference frame of the point cloud.
    #[prost(string, tag="2")]
    pub source: ::prost::alloc::string::String,
    /// the reference frame into which the source data should be transformed, if unset this defaults to the "world" reference frame.
    /// Do not move the robot between the generation of the initial pointcloud and the receipt
    /// of the transformed pointcloud because that will make the transformations inaccurate
    #[prost(string, tag="3")]
    pub destination: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransformPcdResponse {
    #[prost(bytes="vec", tag="1")]
    pub point_cloud_pcd: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceNamesRequest {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceNamesResponse {
    #[prost(message, repeated, tag="1")]
    pub resources: ::prost::alloc::vec::Vec<super::super::common::v1::ResourceName>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceRpcSubtype {
    #[prost(message, optional, tag="1")]
    pub subtype: ::core::option::Option<super::super::common::v1::ResourceName>,
    #[prost(string, tag="2")]
    pub proto_service: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceRpcSubtypesRequest {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceRpcSubtypesResponse {
    #[prost(message, repeated, tag="1")]
    pub resource_rpc_subtypes: ::prost::alloc::vec::Vec<ResourceRpcSubtype>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Operation {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub method: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub arguments: ::core::option::Option<super::super::super::google::protobuf::Struct>,
    #[prost(message, optional, tag="4")]
    pub started: ::core::option::Option<super::super::super::google::protobuf::Timestamp>,
    #[prost(string, optional, tag="5")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOperationsRequest {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOperationsResponse {
    #[prost(message, repeated, tag="1")]
    pub operations: ::prost::alloc::vec::Vec<Operation>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelOperationRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelOperationResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BlockForOperationRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BlockForOperationResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PeerConnectionInfo {
    #[prost(enumeration="PeerConnectionType", tag="1")]
    pub r#type: i32,
    #[prost(string, optional, tag="2")]
    pub remote_address: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag="3")]
    pub local_address: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Session {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub peer_connection_info: ::core::option::Option<PeerConnectionInfo>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSessionsRequest {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSessionsResponse {
    #[prost(message, repeated, tag="1")]
    pub sessions: ::prost::alloc::vec::Vec<Session>,
}
// Discovery
// Discovery is deprecated

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DiscoveryQuery {
    #[prost(string, tag="1")]
    pub subtype: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub model: ::prost::alloc::string::String,
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Discovery {
    #[prost(message, optional, tag="1")]
    pub query: ::core::option::Option<DiscoveryQuery>,
    #[prost(message, optional, tag="2")]
    pub results: ::core::option::Option<super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ModuleModel {
    #[prost(string, tag="1")]
    pub module_name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub model: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub api: ::prost::alloc::string::String,
    #[prost(bool, tag="4")]
    pub from_local_module: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetModelsFromModulesRequest {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetModelsFromModulesResponse {
    #[prost(message, repeated, tag="1")]
    pub models: ::prost::alloc::vec::Vec<ModuleModel>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DiscoverComponentsRequest {
    #[prost(message, repeated, tag="1")]
    pub queries: ::prost::alloc::vec::Vec<DiscoveryQuery>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DiscoverComponentsResponse {
    #[prost(message, repeated, tag="1")]
    pub discovery: ::prost::alloc::vec::Vec<Discovery>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Status {
    #[prost(message, optional, tag="1")]
    pub name: ::core::option::Option<super::super::common::v1::ResourceName>,
    #[prost(message, optional, tag="2")]
    pub status: ::core::option::Option<super::super::super::google::protobuf::Struct>,
    #[prost(message, optional, tag="3")]
    pub last_reconfigured: ::core::option::Option<super::super::super::google::protobuf::Timestamp>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStatusRequest {
    #[prost(message, repeated, tag="1")]
    pub resource_names: ::prost::alloc::vec::Vec<super::super::common::v1::ResourceName>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetStatusResponse {
    #[prost(message, repeated, tag="1")]
    pub status: ::prost::alloc::vec::Vec<Status>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamStatusRequest {
    #[prost(message, repeated, tag="1")]
    pub resource_names: ::prost::alloc::vec::Vec<super::super::common::v1::ResourceName>,
    /// how often to send a new status.
    #[prost(message, optional, tag="2")]
    pub every: ::core::option::Option<super::super::super::google::protobuf::Duration>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamStatusResponse {
    #[prost(message, repeated, tag="1")]
    pub status: ::prost::alloc::vec::Vec<Status>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopExtraParameters {
    #[prost(message, optional, tag="1")]
    pub name: ::core::option::Option<super::super::common::v1::ResourceName>,
    #[prost(message, optional, tag="2")]
    pub params: ::core::option::Option<super::super::super::google::protobuf::Struct>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopAllRequest {
    #[prost(message, repeated, tag="99")]
    pub extra: ::prost::alloc::vec::Vec<StopExtraParameters>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopAllResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StartSessionRequest {
    /// resume can be used to attempt to continue a stream after a disconnection event. If
    /// a session is not found, a new one will be created and returned.
    #[prost(string, tag="1")]
    pub resume: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StartSessionResponse {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub heartbeat_window: ::core::option::Option<super::super::super::google::protobuf::Duration>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SendSessionHeartbeatRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SendSessionHeartbeatResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogRequest {
    #[prost(message, repeated, tag="1")]
    pub logs: ::prost::alloc::vec::Vec<super::super::common::v1::LogEntry>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetCloudMetadataRequest {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetCloudMetadataResponse {
    /// Deprecated: use machine_part_id field.
    #[deprecated]
    #[prost(string, tag="1")]
    pub robot_part_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub primary_org_id: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub location_id: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub machine_id: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub machine_part_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RestartModuleRequest {
    #[prost(oneof="restart_module_request::IdOrName", tags="1, 2")]
    pub id_or_name: ::core::option::Option<restart_module_request::IdOrName>,
}
/// Nested message and enum types in `RestartModuleRequest`.
pub mod restart_module_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum IdOrName {
        /// ID is for registry modules, name for local modules
        #[prost(string, tag="1")]
        ModuleId(::prost::alloc::string::String),
        #[prost(string, tag="2")]
        ModuleName(::prost::alloc::string::String),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RestartModuleResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShutdownRequest {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShutdownResponse {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMachineStatusRequest {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMachineStatusResponse {
    #[prost(message, repeated, tag="1")]
    pub resources: ::prost::alloc::vec::Vec<ResourceStatus>,
    #[prost(message, optional, tag="2")]
    pub config: ::core::option::Option<ConfigStatus>,
    #[prost(enumeration="get_machine_status_response::State", tag="3")]
    pub state: i32,
}
/// Nested message and enum types in `GetMachineStatusResponse`.
pub mod get_machine_status_response {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum State {
        Unspecified = 0,
        /// the machine is reachable but still in the process of configuring initial
        /// modules and resources.
        Initializing = 1,
        /// the machine has finished initializing.
        Running = 2,
    }
    impl State {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                State::Unspecified => "STATE_UNSPECIFIED",
                State::Initializing => "STATE_INITIALIZING",
                State::Running => "STATE_RUNNING",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "STATE_UNSPECIFIED" => Some(Self::Unspecified),
                "STATE_INITIALIZING" => Some(Self::Initializing),
                "STATE_RUNNING" => Some(Self::Running),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceStatus {
    /// resource name.
    #[prost(message, optional, tag="1")]
    pub name: ::core::option::Option<super::super::common::v1::ResourceName>,
    /// current state.
    #[prost(enumeration="resource_status::State", tag="2")]
    pub state: i32,
    /// state transition timestamp.
    #[prost(message, optional, tag="3")]
    pub last_updated: ::core::option::Option<super::super::super::google::protobuf::Timestamp>,
    /// revision of the last config that successfully updated this resource.
    #[prost(string, tag="4")]
    pub revision: ::prost::alloc::string::String,
    /// error details for a resource. This is guaranteed to be null if the
    /// resource is ready and non-null if the resource unhealthy.
    #[prost(string, tag="5")]
    pub error: ::prost::alloc::string::String,
    /// infomation about resource orgID, locationID and partID
    #[prost(message, optional, tag="6")]
    pub cloud_metadata: ::core::option::Option<GetCloudMetadataResponse>,
}
/// Nested message and enum types in `ResourceStatus`.
pub mod resource_status {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum State {
        Unspecified = 0,
        /// a newly created resource.
        Unconfigured = 1,
        /// a resource that is being configured.
        Configuring = 2,
        /// a resource that has been successfully configured once, and is not re-configuring,
        /// being removed, or unhealthy.
        Ready = 3,
        /// a resource that is being removed from the robot.
        Removing = 4,
        /// a resource that is in an unhealthy state.
        Unhealthy = 5,
    }
    impl State {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                State::Unspecified => "STATE_UNSPECIFIED",
                State::Unconfigured => "STATE_UNCONFIGURED",
                State::Configuring => "STATE_CONFIGURING",
                State::Ready => "STATE_READY",
                State::Removing => "STATE_REMOVING",
                State::Unhealthy => "STATE_UNHEALTHY",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "STATE_UNSPECIFIED" => Some(Self::Unspecified),
                "STATE_UNCONFIGURED" => Some(Self::Unconfigured),
                "STATE_CONFIGURING" => Some(Self::Configuring),
                "STATE_READY" => Some(Self::Ready),
                "STATE_REMOVING" => Some(Self::Removing),
                "STATE_UNHEALTHY" => Some(Self::Unhealthy),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfigStatus {
    /// revision of the last config that the machine successfully ingested.
    #[prost(string, tag="1")]
    pub revision: ::prost::alloc::string::String,
    /// config ingestion timestamp.
    #[prost(message, optional, tag="2")]
    pub last_updated: ::core::option::Option<super::super::super::google::protobuf::Timestamp>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVersionRequest {
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVersionResponse {
    /// platform type of viam-server (ie. `rdk` or `micro-rdk`).
    #[prost(string, tag="1")]
    pub platform: ::prost::alloc::string::String,
    /// version of viam-server. If built without a version, it will be dev-<git hash>.
    #[prost(string, tag="2")]
    pub version: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub api_version: ::prost::alloc::string::String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PeerConnectionType {
    Unspecified = 0,
    Grpc = 1,
    Webrtc = 2,
}
impl PeerConnectionType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            PeerConnectionType::Unspecified => "PEER_CONNECTION_TYPE_UNSPECIFIED",
            PeerConnectionType::Grpc => "PEER_CONNECTION_TYPE_GRPC",
            PeerConnectionType::Webrtc => "PEER_CONNECTION_TYPE_WEBRTC",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "PEER_CONNECTION_TYPE_UNSPECIFIED" => Some(Self::Unspecified),
            "PEER_CONNECTION_TYPE_GRPC" => Some(Self::Grpc),
            "PEER_CONNECTION_TYPE_WEBRTC" => Some(Self::Webrtc),
            _ => None,
        }
    }
}
// @@protoc_insertion_point(module)
