use std::string::{FromUtf16Error, FromUtf8Error};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NumberFieldError {
    #[error("field bit size {0} too large for max size {1}")]
    ImproperBitSize(usize, usize),
    #[error("Only 32 bits allowed as size for floats")]
    SizeNotAllowedforF32,
    #[error("{0} field not present in message")]
    FieldNotPresent(String),
    #[error("{0} field was error value")]
    FieldError(String),
}

#[derive(Debug, Error)]
pub enum NmeaParseError {
    #[error(transparent)]
    NumberFieldError(#[from] NumberFieldError),
    #[error(transparent)]
    TryFromSliceError(#[from] std::array::TryFromSliceError),
    #[error("not enough data to parse next field")]
    NotEnoughData,
    #[error("found unsupported PGN {0}")]
    UnsupportedPgn(u32),
    #[error("unknown lookup value for polymorphic field")]
    UnknownPolymorphicLookupValue,
    #[error("unsupported match value encountered")]
    UnsupportedMatchValue,
    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),
    #[error(transparent)]
    FromUtf16Error(#[from] FromUtf16Error),
    #[error("unexpected encoding byte {0} encountered when parsing string")]
    UnexpectedEncoding(u8),
}
