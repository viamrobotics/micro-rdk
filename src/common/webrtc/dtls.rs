use futures_lite::{AsyncRead, AsyncWrite, Future};

use super::io::IoPktChannel;

pub trait DtlsConnector {
    type Stream: AsyncRead + AsyncWrite + Send + Unpin + 'static;
    type Error;
    type Future: Future<Output = Result<Self::Stream, Self::Error>>;

    fn accept(self) -> Self::Future;
    fn set_transport(&mut self, transport: IoPktChannel);
}

pub trait DtlsBuilder {
    type Output: DtlsConnector;
    fn make(&self) -> anyhow::Result<Self::Output>;
}
