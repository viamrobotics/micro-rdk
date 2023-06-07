#![allow(dead_code)]
use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum AttributeError {
    #[error("failed to parse number")]
    ParseNumError,
    #[error("failed to parse number")]
    ConversionImpossibleError,
    #[error("failed to parse number")]
    KeyNotFound,
}

impl From<ParseIntError> for AttributeError {
    fn from(_: ParseIntError) -> AttributeError {
        AttributeError::ParseNumError
    }
}

impl From<ParseFloatError> for AttributeError {
    fn from(_: ParseFloatError) -> AttributeError {
        AttributeError::ParseNumError
    }
}
