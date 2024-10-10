use futures_lite::{AsyncRead, AsyncWrite, Future};

#[cfg(feature = "esp32")]
use crate::esp32::dtls::SSLError;
use thiserror::Error;

use super::udp_mux::UdpMux;

#[derive(Error, Debug)]
pub enum DtlsError {
    #[error(transparent)]
    DtlsError(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[cfg(feature = "esp32")]
    #[error(transparent)]
    DtlsSslError(#[from] SSLError),
}

pub trait DtlsStream: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T> DtlsStream for T where T: AsyncRead + AsyncWrite + Send + Unpin {}
pub trait IntoDtlsStream: Future<Output = Result<Box<dyn DtlsStream>, DtlsError>> {}

pub trait DtlsConnector {
    fn accept(&mut self) -> Result<std::pin::Pin<Box<dyn IntoDtlsStream>>, DtlsError>;
    fn set_transport(&mut self, transport: UdpMux);
}

pub trait DtlsBuilder {
    fn make(&self) -> Result<Box<dyn DtlsConnector>, DtlsError>;
}
