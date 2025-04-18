use crate::google;

use thiserror::Error;

use super::encoder::EncoderError;

#[derive(Error, Debug)]
pub enum StatusError {
    #[error(transparent)]
    EncoderError(#[from] EncoderError),
}

#[deprecated(
    since = "0.5.0",
    note = "Status trait is slated for removal and is not used or required anymore"
)]
pub trait Status {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError>;
}
