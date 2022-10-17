// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Robot {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub location: ::prost::alloc::string::String,
    #[prost(message, optional, tag="4")]
    pub last_access: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="5")]
    pub created_on: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RobotPart {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    /// dns_name part name used for fqdn and local fqdn. Anytime the Name is updated this should be sanitized and updated as well.
    #[prost(string, tag="10")]
    pub dns_name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub secret: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub robot: ::prost::alloc::string::String,
    /// Store the location_id to allow for unique indexes across parts and locations. This filed MUST be updated each time the robots location
    /// changes.
    #[prost(string, tag="12")]
    pub location_id: ::prost::alloc::string::String,
    #[prost(message, optional, tag="5")]
    pub robot_config: ::core::option::Option<::prost_types::Struct>,
    #[prost(message, optional, tag="6")]
    pub last_access: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="7")]
    pub user_supplied_info: ::core::option::Option<::prost_types::Struct>,
    #[prost(bool, tag="8")]
    pub main_part: bool,
    #[prost(string, tag="9")]
    pub fqdn: ::prost::alloc::string::String,
    #[prost(string, tag="11")]
    pub local_fqdn: ::prost::alloc::string::String,
    #[prost(message, optional, tag="13")]
    pub created_on: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RobotPartHistoryEntry {
    #[prost(string, tag="1")]
    pub part: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub robot: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub when: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="4")]
    pub old: ::core::option::Option<RobotPart>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListOrganizationsRequest {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Organization {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub created_on: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListOrganizationsResponse {
    #[prost(message, repeated, tag="1")]
    pub organizations: ::prost::alloc::vec::Vec<Organization>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Location {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub created_on: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListLocationsRequest {
    #[prost(string, tag="1")]
    pub organization_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListLocationsResponse {
    #[prost(message, repeated, tag="1")]
    pub locations: ::prost::alloc::vec::Vec<Location>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LocationAuth {
    #[prost(string, tag="1")]
    pub secret: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LocationAuthRequest {
    #[prost(string, tag="1")]
    pub location_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LocationAuthResponse {
    #[prost(message, optional, tag="1")]
    pub auth: ::core::option::Option<LocationAuth>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotResponse {
    #[prost(message, optional, tag="1")]
    pub robot: ::core::option::Option<Robot>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotPartsRequest {
    #[prost(string, tag="1")]
    pub robot_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotPartsResponse {
    #[prost(message, repeated, tag="1")]
    pub parts: ::prost::alloc::vec::Vec<RobotPart>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotPartRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotPartResponse {
    #[prost(message, optional, tag="1")]
    pub part: ::core::option::Option<RobotPart>,
    #[prost(string, tag="2")]
    pub config_json: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotPartLogsRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(bool, tag="2")]
    pub errors_only: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogEntry {
    #[prost(string, tag="1")]
    pub host: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub level: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(string, tag="4")]
    pub logger_name: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag="6")]
    pub caller: ::core::option::Option<::prost_types::Struct>,
    #[prost(string, tag="7")]
    pub stack: ::prost::alloc::string::String,
    #[prost(message, repeated, tag="8")]
    pub fields: ::prost::alloc::vec::Vec<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotPartLogsResponse {
    #[prost(message, repeated, tag="1")]
    pub logs: ::prost::alloc::vec::Vec<LogEntry>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TailRobotPartLogsRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(bool, tag="2")]
    pub errors_only: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TailRobotPartLogsResponse {
    #[prost(message, repeated, tag="1")]
    pub logs: ::prost::alloc::vec::Vec<LogEntry>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotPartHistoryRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetRobotPartHistoryResponse {
    #[prost(message, repeated, tag="1")]
    pub history: ::prost::alloc::vec::Vec<RobotPartHistoryEntry>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateRobotPartRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub robot_config: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateRobotPartResponse {
    #[prost(message, optional, tag="1")]
    pub part: ::core::option::Option<RobotPart>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NewRobotPartRequest {
    #[prost(string, tag="1")]
    pub robot_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub part_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NewRobotPartResponse {
    #[prost(string, tag="1")]
    pub part_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRobotPartRequest {
    #[prost(string, tag="1")]
    pub part_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRobotPartResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Fragment {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub fragment: ::core::option::Option<::prost_types::Struct>,
    #[prost(string, tag="4")]
    pub organization_owner: ::prost::alloc::string::String,
    #[prost(bool, tag="5")]
    pub public: bool,
    #[prost(message, optional, tag="6")]
    pub created_on: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FindRobotsRequest {
    #[prost(string, tag="1")]
    pub location_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FindRobotsResponse {
    #[prost(message, repeated, tag="1")]
    pub robots: ::prost::alloc::vec::Vec<Robot>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NewRobotRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub location: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NewRobotResponse {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateRobotRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub location: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateRobotResponse {
    #[prost(message, optional, tag="1")]
    pub robot: ::core::option::Option<Robot>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRobotRequest {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRobotResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MarkPartAsMainRequest {
    #[prost(string, tag="1")]
    pub part_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MarkPartAsMainResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RobotConfig {
    #[prost(message, optional, tag="1")]
    pub cloud: ::core::option::Option<CloudConfig>,
    #[prost(message, repeated, tag="2")]
    pub remotes: ::prost::alloc::vec::Vec<RemoteConfig>,
    #[prost(message, repeated, tag="3")]
    pub components: ::prost::alloc::vec::Vec<ComponentConfig>,
    #[prost(message, repeated, tag="4")]
    pub processes: ::prost::alloc::vec::Vec<ProcessConfig>,
    #[prost(message, repeated, tag="5")]
    pub services: ::prost::alloc::vec::Vec<ServiceConfig>,
    #[prost(message, optional, tag="6")]
    pub network: ::core::option::Option<NetworkConfig>,
    #[prost(message, optional, tag="7")]
    pub auth: ::core::option::Option<AuthConfig>,
    /// Turns on debug mode for robot, adding an echo server and more logging and tracing. Only works after restart
    #[prost(bool, optional, tag="8")]
    pub debug: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CloudConfig {
    /// Robot part id.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub fqdn: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub local_fqdn: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub managed_by: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub signaling_address: ::prost::alloc::string::String,
    #[prost(bool, tag="6")]
    pub signaling_insecure: bool,
    #[prost(string, tag="7")]
    pub location_secret: ::prost::alloc::string::String,
    #[prost(string, tag="8")]
    pub secret: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ComponentConfig {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub namespace: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub r#type: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub model: ::prost::alloc::string::String,
    #[prost(message, optional, tag="5")]
    pub frame: ::core::option::Option<Frame>,
    #[prost(string, repeated, tag="6")]
    pub depends_on: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(message, repeated, tag="7")]
    pub service_configs: ::prost::alloc::vec::Vec<ResourceLevelServiceConfig>,
    #[prost(message, optional, tag="8")]
    pub attributes: ::core::option::Option<::prost_types::Struct>,
}
/// A ResourceLevelServiceConfig describes component or remote configuration for a service.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ResourceLevelServiceConfig {
    #[prost(string, tag="1")]
    pub r#type: ::prost::alloc::string::String,
    /// TODO(adam): Should this be move to a structured type as defined in the typescript frontend.
    #[prost(message, optional, tag="2")]
    pub attributes: ::core::option::Option<::prost_types::Struct>,
}
/// A ProcessConfig describes how to manage a system process.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProcessConfig {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="3")]
    pub args: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag="4")]
    pub cwd: ::prost::alloc::string::String,
    #[prost(bool, tag="5")]
    pub one_shot: bool,
    #[prost(bool, tag="6")]
    pub log: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServiceConfig {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub namespace: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub r#type: ::prost::alloc::string::String,
    #[prost(message, optional, tag="4")]
    pub attributes: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NetworkConfig {
    #[prost(string, tag="1")]
    pub fqdn: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub bind_address: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub tls_cert_file: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub tls_key_file: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AuthConfig {
    #[prost(message, repeated, tag="1")]
    pub handlers: ::prost::alloc::vec::Vec<AuthHandlerConfig>,
    #[prost(string, repeated, tag="2")]
    pub tls_auth_entities: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AuthHandlerConfig {
    #[prost(enumeration="CredentialsType", tag="1")]
    pub r#type: i32,
    #[prost(message, optional, tag="5")]
    pub config: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Frame {
    #[prost(string, tag="1")]
    pub parent: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub translation: ::core::option::Option<Translation>,
    #[prost(message, optional, tag="3")]
    pub orientation: ::core::option::Option<Orientation>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Translation {
    #[prost(double, tag="1")]
    pub x: f64,
    #[prost(double, tag="2")]
    pub y: f64,
    #[prost(double, tag="3")]
    pub z: f64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Orientation {
    #[prost(oneof="orientation::Type", tags="1, 2, 3, 4, 5, 6")]
    pub r#type: ::core::option::Option<orientation::Type>,
}
/// Nested message and enum types in `Orientation`.
pub mod orientation {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct NoOrientation {
    }
    /// OrientationVector containing ox, oy, oz, theta represents an orientation vector
    /// Structured similarly to an angle axis, an orientation vector works differently. Rather than representing an orientation
    /// with an arbitrary axis and a rotation around it from an origin, an orientation vector represents orientation
    /// such that the ox/oy/oz components represent the point on the cartesian unit sphere at which your end effector is pointing
    /// from the origin, and that unit vector forms an axis around which theta rotates. This means that incrementing/decrementing
    /// theta will perform an in-line rotation of the end effector.
    /// Theta is defined as rotation between two planes: the plane defined by the origin, the point (0,0,1), and the rx,ry,rz
    /// point, and the plane defined by the origin, the rx,ry,rz point, and the new local Z axis. So if theta is kept at
    /// zero as the north/south pole is circled, the Roll will correct itself to remain in-line.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct OrientationVectorRadians {
        #[prost(double, tag="1")]
        pub theta: f64,
        #[prost(double, tag="2")]
        pub x: f64,
        #[prost(double, tag="3")]
        pub y: f64,
        #[prost(double, tag="4")]
        pub z: f64,
    }
    /// OrientationVectorDegrees is the orientation vector between two objects, but expressed in degrees rather than radians.
    /// Because protobuf Pose is in degrees, this is necessary.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct OrientationVectorDegrees {
        #[prost(double, tag="1")]
        pub theta: f64,
        #[prost(double, tag="2")]
        pub x: f64,
        #[prost(double, tag="3")]
        pub y: f64,
        #[prost(double, tag="4")]
        pub z: f64,
    }
    /// EulerAngles are three angles (in radians) used to represent the rotation of an object in 3D Euclidean space
    /// The Tait–Bryan angle formalism is used, with rotations around three distinct axes in the z-y′-x″ sequence.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct EulerAngles {
        #[prost(double, tag="1")]
        pub roll: f64,
        #[prost(double, tag="2")]
        pub pitch: f64,
        #[prost(double, tag="3")]
        pub yaw: f64,
    }
    /// See here for a thorough explanation: <https://en.wikipedia.org/wiki/Axis%E2%80%93angle_representation>
    /// Basic explanation: Imagine a 3d cartesian grid centered at 0,0,0, and a sphere of radius 1 centered at
    /// that same point. An orientation can be expressed by first specifying an axis, i.e. a line from the origin
    /// to a point on that sphere, represented by (rx, ry, rz), and a rotation around that axis, theta.
    /// These four numbers can be used as-is (R4), or they can be converted to R3, where theta is multiplied by each of
    /// the unit sphere components to give a vector whose length is theta and whose direction is the original axis.
    /// AxisAngles represents an R4 axis angle.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct AxisAngles {
        #[prost(double, tag="1")]
        pub theta: f64,
        #[prost(double, tag="2")]
        pub x: f64,
        #[prost(double, tag="3")]
        pub y: f64,
        #[prost(double, tag="4")]
        pub z: f64,
    }
    /// Quaternion is a float64 precision quaternion.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Quaternion {
        #[prost(double, tag="1")]
        pub w: f64,
        #[prost(double, tag="2")]
        pub x: f64,
        #[prost(double, tag="3")]
        pub y: f64,
        #[prost(double, tag="4")]
        pub z: f64,
    }
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Type {
        #[prost(message, tag="1")]
        NoOrientation(NoOrientation),
        #[prost(message, tag="2")]
        VectorRadians(OrientationVectorRadians),
        #[prost(message, tag="3")]
        VectorDegrees(OrientationVectorDegrees),
        #[prost(message, tag="4")]
        EulerAngles(EulerAngles),
        #[prost(message, tag="5")]
        AxisAngles(AxisAngles),
        #[prost(message, tag="6")]
        Quaternion(Quaternion),
    }
}
/// A RemoteConfig describes a remote robot that should be integrated.
/// The Frame field defines how the "world" node of the remote robot should be reconciled with the "world" node of the
/// the current robot. All components of the remote robot who have Parent as "world" will be attached to the parent defined
/// in Frame, and with the given offset as well.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoteConfig {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub address: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub frame: ::core::option::Option<Frame>,
    #[prost(message, optional, tag="4")]
    pub auth: ::core::option::Option<RemoteAuth>,
    #[prost(string, tag="5")]
    pub managed_by: ::prost::alloc::string::String,
    #[prost(bool, tag="6")]
    pub insecure: bool,
    #[prost(message, optional, tag="7")]
    pub connection_check_interval: ::core::option::Option<::prost_types::Duration>,
    #[prost(message, optional, tag="8")]
    pub reconnect_interval: ::core::option::Option<::prost_types::Duration>,
    #[prost(message, repeated, tag="9")]
    pub service_configs: ::prost::alloc::vec::Vec<ResourceLevelServiceConfig>,
    /// Secret is a helper for a robot location secret.
    #[prost(string, tag="10")]
    pub secret: ::prost::alloc::string::String,
}
/// RemoteAuth specifies how to authenticate against a remote. If no credentials are
/// specified, authentication does not happen. If an entity is specified, the
/// authentication request will specify it.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoteAuth {
    #[prost(message, optional, tag="1")]
    pub credentials: ::core::option::Option<remote_auth::Credentials>,
    #[prost(string, tag="2")]
    pub entity: ::prost::alloc::string::String,
}
/// Nested message and enum types in `RemoteAuth`.
pub mod remote_auth {
    /// Credentials packages up both a type of credential along with its payload which
    /// is formatted specific to the type.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Credentials {
        #[prost(enumeration="super::CredentialsType", tag="1")]
        pub r#type: i32,
        #[prost(string, tag="2")]
        pub payload: ::prost::alloc::string::String,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AgentInfo {
    #[prost(string, tag="1")]
    pub host: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub os: ::prost::alloc::string::String,
    /// list of all ipv4 ips.
    #[prost(string, repeated, tag="3")]
    pub ips: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    /// RDK version
    #[prost(string, tag="4")]
    pub version: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub git_revision: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfigRequest {
    /// Robot part id.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// Details about the RDK (os, version) are updated during this request.
    #[prost(message, optional, tag="2")]
    pub agent_info: ::core::option::Option<AgentInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConfigResponse {
    #[prost(message, optional, tag="1")]
    pub config: ::core::option::Option<RobotConfig>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CertificateRequest {
    /// Robot part id.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CertificateResponse {
    /// Robot part id.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub tls_certificate: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub tls_private_key: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogRequest {
    /// Robot part id.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(message, repeated, tag="2")]
    pub logs: ::prost::alloc::vec::Vec<LogEntry>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NeedsRestartRequest {
    /// Robot part id.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NeedsRestartResponse {
    /// Robot part id.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(bool, tag="2")]
    pub must_restart: bool,
    #[prost(message, optional, tag="3")]
    pub restart_check_interval: ::core::option::Option<::prost_types::Duration>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum CredentialsType {
    Unspecified = 0,
    Internal = 1,
    ApiKey = 2,
    RobotSecret = 3,
    RobotLocationSecret = 4,
}
// @@protoc_insertion_point(module)
