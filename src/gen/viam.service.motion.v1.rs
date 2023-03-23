// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub destination: ::core::option::Option<super::super::super::common::v1::PoseInFrame>,
    #[prost(message, optional, tag="3")]
    pub component_name: ::core::option::Option<super::super::super::common::v1::ResourceName>,
    #[prost(message, optional, tag="4")]
    pub world_state: ::core::option::Option<super::super::super::common::v1::WorldState>,
    #[prost(message, optional, tag="5")]
    pub constraints: ::core::option::Option<Constraints>,
    #[prost(message, optional, tag="6")]
    pub slam_service_name: ::core::option::Option<super::super::super::common::v1::ResourceName>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveResponse {
    #[prost(bool, tag="1")]
    pub success: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveSingleComponentRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag="2")]
    pub destination: ::core::option::Option<super::super::super::common::v1::PoseInFrame>,
    #[prost(message, optional, tag="3")]
    pub component_name: ::core::option::Option<super::super::super::common::v1::ResourceName>,
    #[prost(message, optional, tag="4")]
    pub world_state: ::core::option::Option<super::super::super::common::v1::WorldState>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveSingleComponentResponse {
    #[prost(bool, tag="1")]
    pub success: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPoseRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// the component whose pose is being requested
    #[prost(message, optional, tag="2")]
    pub component_name: ::core::option::Option<super::super::super::common::v1::ResourceName>,
    /// the reference frame in which the component's pose
    /// should be provided, if unset this defaults
    /// to the "world" reference frame
    #[prost(string, tag="3")]
    pub destination_frame: ::prost::alloc::string::String,
    /// pose information on any additional reference frames that are needed
    /// to compute the component's pose
    #[prost(message, repeated, tag="4")]
    pub supplemental_transforms: ::prost::alloc::vec::Vec<super::super::super::common::v1::Transform>,
    /// Additional arguments to the method
    #[prost(message, optional, tag="99")]
    pub extra: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPoseResponse {
    #[prost(message, optional, tag="1")]
    pub pose: ::core::option::Option<super::super::super::common::v1::PoseInFrame>,
}
/// Constraints specifies all enumerated constraints to be passed to Viam's motion planning, along with any optional parameters
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Constraints {
    /// Typed message for a specific constraint
    #[prost(message, repeated, tag="1")]
    pub linear_constraint: ::prost::alloc::vec::Vec<LinearConstraint>,
    #[prost(message, repeated, tag="2")]
    pub orientation_constraint: ::prost::alloc::vec::Vec<OrientationConstraint>,
    /// Arc constraint, Time constraint, and others will be added here when they are supported
    #[prost(message, repeated, tag="3")]
    pub collision_specification: ::prost::alloc::vec::Vec<CollisionSpecification>,
}
/// LinearConstraint specifies that the component being moved should move linearly relative to its goal. It does not constrain the motion of components other than the `component_name` specified in motion.Move
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LinearConstraint {
    /// Max linear deviation from straight-line between start and goal, in mm.
    #[prost(float, optional, tag="1")]
    pub line_tolerance_mm: ::core::option::Option<f32>,
    /// Max allowable orientation deviation, in degrees, while on the shortest path between start / goal states
    #[prost(float, optional, tag="2")]
    pub orientation_tolerance_degs: ::core::option::Option<f32>,
}
/// OrientationConstraint specifies that the component being moved will not deviate its orientation beyond some threshold relative to the goal. It does not constrain the motion of components other than the `component_name` specified in motion.Move
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrientationConstraint {
    /// Max allowable orientation deviation, in degrees, while on the shortest path between start / goal states
    #[prost(float, optional, tag="1")]
    pub orientation_tolerance_degs: ::core::option::Option<f32>,
}
/// CollisionSpecification is used to selectively apply obstacle avoidance to specific parts of the robot
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CollisionSpecification {
    /// Pairs of frame which should be allowed to collide with one another
    #[prost(message, repeated, tag="1")]
    pub allows: ::prost::alloc::vec::Vec<collision_specification::AllowedFrameCollisions>,
}
/// Nested message and enum types in `CollisionSpecification`.
pub mod collision_specification {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct AllowedFrameCollisions {
        #[prost(string, tag="1")]
        pub frame1: ::prost::alloc::string::String,
        #[prost(string, tag="2")]
        pub frame2: ::prost::alloc::string::String,
    }
}
// @@protoc_insertion_point(module)
