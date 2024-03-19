use futures_lite::{AsyncRead, AsyncWrite, Future};

use super::io::IoPktChannel;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DtlsError {
    #[error(transparent)]
    DtlsError(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub trait DtlsConnector {
    type Stream: AsyncRead + AsyncWrite + Send + Unpin + 'static;
    type Error: std::error::Error + Send + Sync + 'static;
    type Future: Future<Output = Result<Self::Stream, Self::Error>>;

    fn accept(self) -> Result<Self::Future, Self::Error>;
    fn set_transport(&mut self, transport: IoPktChannel);
}

pub trait DtlsBuilder {
    type Output: DtlsConnector;
    fn make(&self) -> Result<Self::Output, DtlsError>;
}
