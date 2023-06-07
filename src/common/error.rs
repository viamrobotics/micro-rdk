#![allow(dead_code)]
use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum AttributeError {
    #[error("failed to parse number")]
    ParseNumError,
    #[error("value not possible")]
    ConversionImpossibleError,
    #[error("attribute `{0}` was not found")]
    KeyNotFound(String),
    #[error("config has no attribute map")]
    NoAttributeMap,
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
