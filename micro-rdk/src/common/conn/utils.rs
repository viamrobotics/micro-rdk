use std::{convert::Infallible, io, pin::Pin, task::Poll};

use futures_lite::Future;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::common::webrtc::{
    certificate::{Certificate, Fingerprint},
    dtls::{DtlsBuilder, DtlsConnector},
};

use super::{
    errors::ServerError,
    server::{AsyncableTcpListener, Http2Connector, OwnedListener, TlsClientConnector},
};

#[derive(Default)]
pub struct WebRtcNoOp {
    fp: Fingerprint,
}

impl AsyncRead for WebRtcNoOp {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}

impl AsyncWrite for WebRtcNoOp {
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Pending
    }
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Poll::Pending
    }
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Pending
    }
}

impl futures_lite::AsyncRead for WebRtcNoOp {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Pending
    }
}
impl futures_lite::AsyncWrite for WebRtcNoOp {
    fn poll_close(self: Pin<&mut Self>, _: &mut std::task::Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Pending
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut std::task::Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}

#[derive(Debug)]
pub struct NoHttp2 {}
impl AsyncRead for NoHttp2 {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}
impl AsyncWrite for NoHttp2 {
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Pending
    }
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Pending
    }
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Poll::Pending
    }
}

impl TlsClientConnector for WebRtcNoOp {
    type Stream = WebRtcNoOp;
    fn connect(&mut self) -> Result<Self::Stream, ServerError> {
        Err(ServerError::ServerConnectionNotConfigured)
    }
}

impl DtlsBuilder for WebRtcNoOp {
    type Output = WebRtcNoOp;
    fn make(&self) -> anyhow::Result<Self::Output> {
        Ok(WebRtcNoOp::default())
    }
}

impl DtlsConnector for WebRtcNoOp {
    type Error = Infallible;
    type Stream = WebRtcNoOp;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Stream, Self::Error>>>>;
    fn accept(self) -> Result<Self::Future, Self::Error> {
        Ok(Box::pin(futures_lite::future::pending()))
    }
    fn set_transport(&mut self, _: crate::common::webrtc::io::IoPktChannel) {}
}

impl Certificate for WebRtcNoOp {
    fn get_der_certificate(&self) -> &'_ [u8] {
        &[0_u8; 0]
    }
    fn get_der_keypair(&self) -> &'_ [u8] {
        &[0_u8; 0]
    }
    fn get_fingerprint(&self) -> &'_ crate::common::webrtc::certificate::Fingerprint {
        &self.fp
    }
}

impl Http2Connector for NoHttp2 {
    type Stream = NoHttp2;
    fn accept(&mut self) -> std::io::Result<Self::Stream> {
        Err(io::Error::from(io::ErrorKind::NotConnected))
    }
}

impl AsyncableTcpListener<NoHttp2> for NoHttp2 {
    type Output = NoHttp2;
    fn as_async_listener(&self) -> OwnedListener<Self::Output> {
        let pend = futures_lite::future::pending::<io::Result<NoHttp2>>();
        OwnedListener {
            inner: Box::pin(pend),
        }
    }
}
