use thiserror::Error;

#[derive(Debug, Error)]
pub enum NumberFieldError {
    #[error("field bit size {0} too large for max size {0}")]
    ImproperBitSize(usize, usize),
}

#[derive(Debug, Error)]
pub enum NmeaParseError {
    #[error(transparent)]
    NumberFieldError(#[from] NumberFieldError),
    #[error(transparent)]
    TryFromSliceError(#[from] std::array::TryFromSliceError),
    #[error("end of buffer exceeded")]
    EndOfBufferExceeded,
    #[error("found unsupported PGN {0}")]
    UnsupportedPgn(u32),
}
