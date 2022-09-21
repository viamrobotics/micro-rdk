// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetModelParameterSchemaRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// name of the type of vision model
    #[prost(string, tag="2")]
    pub model_type: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetModelParameterSchemaResponse {
    /// the parameters as JSON bytes of a jsonschema.Schema
    #[prost(bytes="vec", tag="1")]
    pub model_parameter_schema: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDetectorNamesRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDetectorNamesResponse {
    /// detectors in the registry
    #[prost(string, repeated, tag="1")]
    pub detector_names: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddDetectorRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub detector_name: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub detector_model_type: ::prost::alloc::string::String,
    #[prost(message, optional, tag="4")]
    pub detector_parameters: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddDetectorResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveDetectorRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// name of detector in registry
    #[prost(string, tag="2")]
    pub detector_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveDetectorResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDetectionsRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// the image, encoded as bytes
    #[prost(bytes="vec", tag="2")]
    pub image: ::prost::alloc::vec::Vec<u8>,
    /// the width of the image
    #[prost(int64, tag="3")]
    pub width: i64,
    /// the height of the image
    #[prost(int64, tag="4")]
    pub height: i64,
    /// the actual MIME type of image
    #[prost(string, tag="5")]
    pub mime_type: ::prost::alloc::string::String,
    /// name of the registered detector to use
    #[prost(string, tag="6")]
    pub detector_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDetectionsResponse {
    /// the bounding boxes and labels
    #[prost(message, repeated, tag="1")]
    pub detections: ::prost::alloc::vec::Vec<Detection>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDetectionsFromCameraRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// name of camera source to use as input
    #[prost(string, tag="2")]
    pub camera_name: ::prost::alloc::string::String,
    /// name of the registered detector to use
    #[prost(string, tag="3")]
    pub detector_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDetectionsFromCameraResponse {
    /// the bounding boxes and labels
    #[prost(message, repeated, tag="1")]
    pub detections: ::prost::alloc::vec::Vec<Detection>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Detection {
    /// the four corners of the box
    #[prost(int64, optional, tag="1")]
    pub x_min: ::core::option::Option<i64>,
    #[prost(int64, optional, tag="2")]
    pub y_min: ::core::option::Option<i64>,
    #[prost(int64, optional, tag="3")]
    pub x_max: ::core::option::Option<i64>,
    #[prost(int64, optional, tag="4")]
    pub y_max: ::core::option::Option<i64>,
    /// the confidence of the detection
    #[prost(double, tag="5")]
    pub confidence: f64,
    /// label associated with the detected object
    #[prost(string, tag="6")]
    pub class_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClassifierNamesRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClassifierNamesResponse {
    #[prost(string, repeated, tag="1")]
    pub classifier_names: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddClassifierRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// name of classifier to add to registry
    #[prost(string, tag="2")]
    pub classifier_name: ::prost::alloc::string::String,
    /// the type of classifier
    #[prost(string, tag="3")]
    pub classifier_model_type: ::prost::alloc::string::String,
    /// additional parameters
    #[prost(message, optional, tag="4")]
    pub classifier_parameters: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddClassifierResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveClassifierRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// name of the classifier in registry
    #[prost(string, tag="2")]
    pub classifier_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveClassifierResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClassificationsRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// the image encoded as bytes
    #[prost(bytes="vec", tag="2")]
    pub image: ::prost::alloc::vec::Vec<u8>,
    /// the width of the image
    #[prost(int32, tag="3")]
    pub width: i32,
    /// the height of the image
    #[prost(int32, tag="4")]
    pub height: i32,
    /// the actual MIME type of image
    #[prost(string, tag="5")]
    pub mime_type: ::prost::alloc::string::String,
    /// the name of the registered classifier
    #[prost(string, tag="6")]
    pub classifier_name: ::prost::alloc::string::String,
    /// the number of classifications desired
    #[prost(int32, tag="7")]
    pub n: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClassificationsResponse {
    #[prost(message, repeated, tag="1")]
    pub classifications: ::prost::alloc::vec::Vec<Classification>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClassificationsFromCameraRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// the image encoded as bytes
    #[prost(string, tag="2")]
    pub camera_name: ::prost::alloc::string::String,
    /// the name of the registered classifier
    #[prost(string, tag="3")]
    pub classifier_name: ::prost::alloc::string::String,
    /// the number of classifications desired
    #[prost(int32, tag="4")]
    pub n: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClassificationsFromCameraResponse {
    #[prost(message, repeated, tag="1")]
    pub classifications: ::prost::alloc::vec::Vec<Classification>,
}
/// the general form of the output from a classifier
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Classification {
    /// the class name
    #[prost(string, tag="1")]
    pub class_name: ::prost::alloc::string::String,
    /// the confidence score of the classification
    #[prost(double, tag="2")]
    pub confidence: f64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSegmenterNamesRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSegmenterNamesResponse {
    /// segmenters in the registry
    #[prost(string, repeated, tag="1")]
    pub segmenter_names: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddSegmenterRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// name of the segmenter
    #[prost(string, tag="2")]
    pub segmenter_name: ::prost::alloc::string::String,
    /// name of the segmenter model
    #[prost(string, tag="3")]
    pub segmenter_model_type: ::prost::alloc::string::String,
    /// parameters of the segmenter model
    #[prost(message, optional, tag="4")]
    pub segmenter_parameters: ::core::option::Option<::prost_types::Struct>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddSegmenterResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveSegmenterRequest {
    /// name of the vision service
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// name of segmenter in registry
    #[prost(string, tag="2")]
    pub segmenter_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveSegmenterResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetObjectPointCloudsRequest {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Name of a camera
    #[prost(string, tag="2")]
    pub camera_name: ::prost::alloc::string::String,
    /// Name of the segmentation algorithm
    #[prost(string, tag="3")]
    pub segmenter_name: ::prost::alloc::string::String,
    /// Requested MIME type of response
    #[prost(string, tag="4")]
    pub mime_type: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetObjectPointCloudsResponse {
    /// Actual MIME type of response
    #[prost(string, tag="1")]
    pub mime_type: ::prost::alloc::string::String,
    /// List of objects in the scene
    #[prost(message, repeated, tag="2")]
    pub objects: ::prost::alloc::vec::Vec<super::super::super::common::v1::PointCloudObject>,
}
// @@protoc_insertion_point(module)
