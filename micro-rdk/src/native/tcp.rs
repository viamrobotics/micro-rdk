use crate::common::conn::server::Http2Connector;
use crate::common::conn::viam::{HTTP2Stream, IntoHttp2Stream, ViamH2Connector};
use crate::native::tls::{NativeTls, NativeTlsStream};
use async_io::Async;
use futures_lite::future::FutureExt;

use futures_lite::{ready, Future};
use futures_rustls::{TlsAcceptor, TlsConnector};
use hyper::{rt, Uri};
use rustls::{ClientConfig, OwnedTrustAnchor, RootCertStore, ServerConfig};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::fmt::Debug;
use std::io::{self, BufReader};
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::pin::Pin;

use std::sync::Arc;
use std::{
    marker::PhantomData,
    net::{TcpListener, TcpStream},
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
        if self.srv_cert.is_some() && self.srv_key.is_some() {
            let cert_chain = rustls_pemfile::certs(&mut BufReader::new(
                self.srv_cert.as_ref().unwrap().as_slice(),
            ))
            .unwrap()
            .iter()
            .map(|c| rustls::Certificate(c.clone()))
            .collect();
            let priv_keys: Vec<_> = rustls_pemfile::pkcs8_private_keys(&mut BufReader::new(
                self.srv_key.as_ref().unwrap().as_slice(),
            ))
            .unwrap()
            .iter()
            .map(|k| rustls::PrivateKey(k.clone()))
            .collect();
            let mut cfg = ServerConfig::builder()
                .with_safe_default_cipher_suites()
                .with_safe_default_kx_groups()
                .with_protocol_versions(&[&rustls::version::TLS12])
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?
                .with_no_client_auth()
                .with_single_cert(cert_chain, priv_keys[0].clone())
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
            cfg.alpn_protocols = vec!["h2".as_bytes().to_vec()];
            return Ok(Box::pin(NativeStreamAcceptor(
                TlsAcceptor::from(Arc::new(cfg)).accept(connection),
            )));
        } else {
            return Ok(Box::pin(NativeStreamInsecureAcceptor(Some(connection))));
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
        let mut root_certs = RootCertStore::empty();
        root_certs.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
            OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));
        let mut cfg = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_certs)
            .with_no_client_auth();
        cfg.alpn_protocols = vec!["h2".as_bytes().to_vec()];
        let stream = async_io::Async::new(TcpStream::connect("app.viam.com:443")?).unwrap();
        let conn = TlsConnector::from(Arc::new(cfg));
        Ok(Box::pin(NativeStreamConnector(
            conn.connect(
                "app.viam.com"
                    .try_into()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                    .unwrap(),
                stream,
            ),
        )))
    }
}

pub struct NativeStreamConnector(futures_rustls::Connect<Async<TcpStream>>);
impl IntoHttp2Stream for NativeStreamConnector {}

impl Future for NativeStreamConnector {
    type Output = Result<Box<dyn HTTP2Stream>, std::io::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result: Self::Output = futures_lite::ready!(self.0.poll(cx))
            .map(|e| Box::new(NativeStream::NewTlsStream(e.into())) as Box<dyn HTTP2Stream>);
        Poll::Ready(result)
    }
}

pub struct NativeStreamAcceptor(futures_rustls::Accept<Async<TcpStream>>);
impl IntoHttp2Stream for NativeStreamAcceptor {}

impl Future for NativeStreamAcceptor {
    type Output = Result<Box<dyn HTTP2Stream>, std::io::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result: Self::Output = futures_lite::ready!(self.0.poll(cx))
            .map(|e| Box::new(NativeStream::NewTlsStream(e.into())) as Box<dyn HTTP2Stream>);
        Poll::Ready(result)
    }
}

pub struct NativeStreamInsecureAcceptor(Option<Async<TcpStream>>);
impl IntoHttp2Stream for NativeStreamInsecureAcceptor {}

impl Future for NativeStreamInsecureAcceptor {
    type Output = Result<Box<dyn HTTP2Stream>, std::io::Error>;
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(Ok(Box::new(NativeStream::LocalPlain(
            self.0.take().unwrap().into(),
        ))))
    }
}

/// Struct to listen for incoming TCP connections
pub struct NativeListener {
    listener: TcpListener,
    #[allow(dead_code)]
    addr: SockAddr,
    _marker: PhantomData<*const ()>,
    tls: Option<Box<NativeTls>>,
}

impl NativeListener {
    /// Creates a new Tcplistener
    pub fn new(addr: SockAddr, tls: Option<Box<NativeTls>>) -> Result<Self, std::io::Error> {
        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
        socket.set_reuse_address(true)?;
        socket.set_nodelay(true)?;
        socket.bind(&addr)?;
        socket.listen(128)?;
        Ok(Self {
            listener: socket.into(),
            addr,
            _marker: PhantomData,
            tls,
        })
    }
}

pub struct NativeIncoming {
    tls: Option<Box<NativeTls>>,
    inner: futures_lite::future::Boxed<io::Result<(Async<TcpStream>, SocketAddr)>>,
}

impl Future for NativeIncoming {
    type Output = io::Result<NativeTlsConnector>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let r = match ready!(self.inner.poll(cx)) {
            Ok(r) => NativeTlsConnector {
                inner: r.0.into_inner().unwrap(),
                tls: self.tls.clone(),
            },
            Err(e) => return Poll::Ready(Err(e)),
        };
        Poll::Ready(Ok(r))
    }
}

pub struct NativeTlsConnector {
    tls: Option<Box<NativeTls>>,
    inner: TcpStream,
}

impl Debug for NativeTlsConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeTlsConnector")
            .field("inner", &self.inner)
            .field("tls", &self.tls.is_some())
            .finish()
    }
}

impl Http2Connector for NativeTlsConnector {
    type Stream = NativeStream;
    async fn accept(&mut self) -> std::io::Result<Self::Stream> {
        match self.tls.as_ref() {
            Some(tls) => tls
                .open_ssl_context(Some(self.inner.try_clone().unwrap()))
                .await
                .map(|s| NativeStream::TLSStream(Box::new(s)))
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
            None => Ok(NativeStream::LocalPlain(
                async_io::Async::new(self.inner.try_clone().unwrap()).unwrap(),
            )),
        }
    }
}

/// Enum to represent a TCP stream (either plain or encrypted)
pub enum NativeStream {
    LocalPlain(Async<TcpStream>),
    TLSStream(Box<NativeTlsStream>),
    NewTlsStream(futures_rustls::TlsStream<Async<TcpStream>>),
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
            NativeStream::TLSStream(s) => {
                futures_lite::pin!(s);
                match ready!(s.poll_read(cx, uninit_buf)) {
                    Ok(s) => {
                        unsafe { buf.advance(s) };
                        Poll::Ready(Ok(()))
                    }

                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            NativeStream::NewTlsStream(s) => {
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
            NativeStream::TLSStream(s) => {
                futures_lite::pin!(s);
                match ready!(s.poll_write(cx, buf)) {
                    Ok(s) => Poll::Ready(Ok(s)),
                    Err(_) => Poll::Pending,
                }
            }
            NativeStream::NewTlsStream(s) => {
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
            NativeStream::TLSStream(s) => {
                futures_lite::pin!(s);
                s.poll_flush(cx)
            }
            NativeStream::NewTlsStream(s) => {
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
            NativeStream::TLSStream(s) => {
                futures_lite::pin!(s);
                s.poll_close(cx)
            }
            NativeStream::NewTlsStream(s) => {
                futures_lite::pin!(s);
                s.poll_close(cx)
            }
        }
    }
}
