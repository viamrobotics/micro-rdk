use std::sync::{Arc, Mutex};

use crate::google;

use thiserror::Error;

use super::encoder::EncoderError;

#[derive(Error, Debug)]
pub enum StatusError {
    #[error(transparent)]
    EncoderError(#[from] EncoderError),
}

pub trait Status {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError>;
}

impl<L> Status for Mutex<L>
where
    L: ?Sized + Status,
{
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        self.lock().unwrap().get_status()
    }
}

impl<A> Status for Arc<A>
where
    A: ?Sized + Status,
{
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        (**self).get_status()
    }
}
