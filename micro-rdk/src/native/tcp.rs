use crate::common::conn::viam::{HTTP2Stream, IntoHttp2Stream, ViamH2Connector};
use async_io::Async;
use futures_lite::future::FutureExt;

use futures_lite::{Future, ready};
use futures_rustls::{TlsAcceptor, TlsConnector};
use hyper::{Uri, rt};
use rustls::{ClientConfig, KeyLogFile, RootCertStore, ServerConfig};
use std::io::BufReader;
use std::mem::MaybeUninit;
use std::pin::Pin;

use std::sync::Arc;
use std::{
    net::TcpStream,
    task::{Context, Poll},
};

#[derive(Default)]
pub struct NativeH2Connector {
    srv_cert: Option<Vec<u8>>,
    srv_key: Option<Vec<u8>>,
}

impl ViamH2Connector for NativeH2Connector {
    fn accept_connection(
        &self,
        connection: Async<TcpStream>,
    ) -> Result<std::pin::Pin<Box<dyn IntoHttp2Stream>>, std::io::Error> {
        if let (Some(srv_cert), Some(srv_key)) = (&self.srv_cert, &self.srv_key) {
            let cert_chain = rustls_pemfile::certs(&mut BufReader::new(srv_cert.as_slice()))
                .map(|c| c.unwrap().into_owned())
                .collect();
            let priv_key = rustls_pemfile::private_key(&mut BufReader::new(srv_key.as_slice()))
                .unwrap()
                .ok_or_else(|| std::io::Error::other("no private key found"))?;
            let mut cfg = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS12])
                .with_no_client_auth()
                .with_single_cert(cert_chain, priv_key)
                .map_err(std::io::Error::other)?;
            cfg.alpn_protocols = vec!["h2".as_bytes().to_vec()];
            Ok(Box::pin(NativeStreamAcceptor(
                TlsAcceptor::from(Arc::new(cfg)).accept(connection),
            )))
        } else {
            Ok(Box::pin(NativeStreamInsecureAcceptor(Some(connection))))
        }
    }
    fn set_server_certificates(&mut self, srv_cert: Vec<u8>, srv_key: Vec<u8>) {
        let _ = self.srv_cert.replace(srv_cert);
        let _ = self.srv_key.replace(srv_key);
    }
    fn connect_to(
        &self,
        uri: &Uri,
    ) -> Result<std::pin::Pin<Box<dyn IntoHttp2Stream>>, std::io::Error> {
        if uri.scheme_str().is_some_and(|s| s == "http") {
            log::info!("insecurely connecting to {:?}", uri);
            let stream =
                async_io::Async::new(TcpStream::connect(uri.authority().unwrap().as_str())?)
                    .unwrap();
            return Ok(Box::pin(NativeStreamInsecureAcceptor(Some(stream))));
        }
        let root_certs = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let mut cfg = ClientConfig::builder()
            .with_root_certificates(root_certs)
            .with_no_client_auth();
        let log = Arc::new(KeyLogFile::new());
        cfg.key_log = log;
        cfg.alpn_protocols = vec!["h2".as_bytes().to_vec()];
        let stream =
            async_io::Async::new(TcpStream::connect(uri.authority().unwrap().as_str())?).unwrap();
        let conn = TlsConnector::from(Arc::new(cfg));
        let server_name: rustls::pki_types::ServerName<'static> = uri
            .host()
            .unwrap()
            .to_owned()
            .try_into()
            .map_err(std::io::Error::other)?;
        Ok(Box::pin(NativeStreamConnector(
            conn.connect(server_name, stream),
        )))
    }
}

pub struct NativeStreamConnector(futures_rustls::Connect<Async<TcpStream>>);
impl IntoHttp2Stream for NativeStreamConnector {}

impl Future for NativeStreamConnector {
    type Output = Result<Box<dyn HTTP2Stream>, std::io::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result: Self::Output = futures_lite::ready!(self.0.poll(cx))
            .map(|e| Box::new(NativeStream::TlsStream(e.into())) as Box<dyn HTTP2Stream>);
        Poll::Ready(result)
    }
}

pub struct NativeStreamAcceptor(futures_rustls::Accept<Async<TcpStream>>);
impl IntoHttp2Stream for NativeStreamAcceptor {}

impl Future for NativeStreamAcceptor {
    type Output = Result<Box<dyn HTTP2Stream>, std::io::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result: Self::Output = futures_lite::ready!(self.0.poll(cx))
            .map(|e| Box::new(NativeStream::TlsStream(e.into())) as Box<dyn HTTP2Stream>);
        Poll::Ready(result)
    }
}

pub struct NativeStreamInsecureAcceptor(Option<Async<TcpStream>>);
impl IntoHttp2Stream for NativeStreamInsecureAcceptor {}

impl Future for NativeStreamInsecureAcceptor {
    type Output = Result<Box<dyn HTTP2Stream>, std::io::Error>;
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(Ok(Box::new(NativeStream::LocalPlain(
            self.0.take().unwrap(),
        ))))
    }
}

/// Enum to represent a TCP stream (either plain or encrypted)
#[allow(clippy::large_enum_variant)] // TODO: Box the TlsStream
pub enum NativeStream {
    LocalPlain(Async<TcpStream>),
    TlsStream(futures_rustls::TlsStream<Async<TcpStream>>),
}

use futures_lite::{AsyncRead, AsyncWrite};
impl rt::Read for NativeStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let uninit_buf = unsafe { &mut *(buf.as_mut() as *mut [MaybeUninit<u8>] as *mut [u8]) };
        match &mut *self {
            NativeStream::LocalPlain(s) => {
                futures_lite::pin!(s);
                match ready!(s.poll_read(cx, uninit_buf)) {
                    Ok(s) => {
                        unsafe { buf.advance(s) };
                        Poll::Ready(Ok(()))
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }

            NativeStream::TlsStream(s) => {
                futures_lite::pin!(s);
                match ready!(s.poll_read(cx, uninit_buf)) {
                    Ok(s) => {
                        unsafe { buf.advance(s) };
                        Poll::Ready(Ok(()))
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
        }
    }
}

impl rt::Write for NativeStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match &mut *self {
            NativeStream::LocalPlain(s) => {
                futures_lite::pin!(s);

                match ready!(s.poll_write(cx, buf)) {
                    Ok(s) => Poll::Ready(Ok(s)),
                    Err(_) => Poll::Pending,
                }
            }

            NativeStream::TlsStream(s) => {
                futures_lite::pin!(s);
                match ready!(s.poll_write(cx, buf)) {
                    Ok(s) => Poll::Ready(Ok(s)),
                    Err(_) => Poll::Pending,
                }
            }
        }
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match &mut *self {
            NativeStream::LocalPlain(s) => {
                futures_lite::pin!(s);
                s.poll_flush(cx)
            }

            NativeStream::TlsStream(s) => {
                futures_lite::pin!(s);
                s.poll_flush(cx)
            }
        }
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match &mut *self {
            NativeStream::LocalPlain(s) => {
                futures_lite::pin!(s);
                s.poll_close(cx)
            }

            NativeStream::TlsStream(s) => {
                futures_lite::pin!(s);
                s.poll_close(cx)
            }
        }
    }
}
