use std::{convert::Infallible, io, pin::Pin, task::Poll};

use futures_lite::Future;
use hyper::rt;

use crate::common::webrtc::{
    certificate::{Certificate, Fingerprint},
    dtls::{DtlsBuilder, DtlsConnector, DtlsError},
    udp_mux::UdpMux,
};

use super::{
    errors::ServerError,
    server::{AsyncableTcpListener, Http2Connector, OwnedListener, TlsClientConnector},
};

#[derive(Default)]
pub struct WebRtcNoOp {
    fp: Fingerprint,
}

impl rt::Read for WebRtcNoOp {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Pending
    }
}

impl rt::Write for WebRtcNoOp {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Poll::Pending
    }
    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Pending
    }
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
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

impl rt::Read for NoHttp2 {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Pending
    }
}

impl rt::Write for NoHttp2 {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Poll::Pending
    }
    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Pending
    }
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Pending
    }
}

impl TlsClientConnector for WebRtcNoOp {
    type Stream = WebRtcNoOp;
    async fn connect(&mut self) -> Result<Self::Stream, ServerError> {
        Err(ServerError::ServerConnectionNotConfigured)
    }
}

impl DtlsBuilder for WebRtcNoOp {
    type Output = WebRtcNoOp;
    fn make(&self) -> Result<Self::Output, DtlsError> {
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
    fn set_transport(&mut self, _: UdpMux) {}
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
    async fn accept(&mut self) -> std::io::Result<Self::Stream> {
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
