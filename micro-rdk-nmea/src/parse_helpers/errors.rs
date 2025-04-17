use thiserror::Error;

#[derive(Debug, Error)]
pub enum NumberFieldError {
    #[error("field bit size {0} too large for max size {0}")]
    ImproperBitSize(usize, usize),
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
    #[error("could not parse timestamp")]
    MalformedTimestamp,
    #[error("found unsupported PGN {0}")]
    UnsupportedPgn(u32),
    #[error("unknown lookup value for polymorphic field")]
    UnknownPolymorphicLookupValue,
}
