use crate::common::conn::viam::{HTTP2Stream, IntoHttp2Stream, ViamH2Connector};
use async_io::Async;

use esp_idf_svc::sys::{
    esp, esp_crt_bundle_attach, esp_tls_cfg, esp_tls_cfg_server, esp_tls_conn_destroy,
    esp_tls_get_ssl_context, esp_tls_init, esp_tls_server_session_create, esp_tls_t,
    mbedtls_ssl_conf_read_timeout, mbedtls_ssl_config, mbedtls_ssl_context, EspError,
};
use futures_lite::FutureExt;
use futures_lite::{ready, AsyncRead, AsyncWrite, Future};
use hyper::{rt, Uri};
use std::ffi::{c_char, c_void, CString};
use std::mem::{self, MaybeUninit};

use std::ops::Deref;
use std::os::fd::AsRawFd;
use std::pin::Pin;
use std::sync::Arc;
use std::{
    net::TcpStream,
    task::{Context, Poll},
};

use super::dtls::{AsyncSSLStream, SSLContext, SSLError};

extern "C" {
    fn esp_create_mbedtls_handle(
        hostname: *const c_char,
        hostlen: i32,
        cfg: *const c_void,
        tls: *mut esp_tls_t,
    ) -> i32;
}

pub(crate) static ALPN_PROTOCOLS: &[u8] = b"h2\0";

pub(crate) struct Esp32TLSContext(pub(crate) *mut esp_tls_t);

impl Esp32TLSContext {
    pub(crate) fn new() -> Result<Self, std::io::Error> {
        let p = unsafe { esp_tls_init() };
        if p.is_null() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "couldn't allocate tls context",
            ));
        }
        Ok(Self(p))
    }
}

impl Deref for Esp32TLSContext {
    type Target = *mut esp_tls_t;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Drop for Esp32TLSContext {
    fn drop(&mut self) {
        if let Some(err) = EspError::from(unsafe { esp_tls_conn_destroy(self.0) }) {
            log::error!("error while dropping the tls connection '{}'", err);
        }
    }
}

#[allow(dead_code)]
struct Esp32ServerConfig {
    cfg: Box<esp_tls_cfg_server>,
    alpn_proto: Vec<*const c_char>,
}

impl Esp32ServerConfig {
    fn new(srv_cert: &[u8], srv_key: &[u8]) -> Self {
        let mut alpn_proto: Vec<_> = vec![ALPN_PROTOCOLS.as_ptr() as *const i8, std::ptr::null()];
        // TODO(RSDK-10200): Missing fields when using ESP-IDF 5
        let cfg = Box::new(esp_tls_cfg_server {
            alpn_protos: alpn_proto.as_mut_ptr(),
            __bindgen_anon_1: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_1 {
                // The CA root is not need when a client is connecting as it's available
                cacert_buf: std::ptr::null(),
            },
            __bindgen_anon_2: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_2 {
                cacert_bytes: 0,
            },
            __bindgen_anon_3: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_3 {
                // This is the server certificates in the PEM format
                servercert_buf: srv_cert.as_ptr(),
            },
            __bindgen_anon_4: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_4 {
                servercert_bytes: srv_cert.len() as u32,
            },
            __bindgen_anon_5: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_5 {
                serverkey_buf: srv_key.as_ptr(),
            },
            __bindgen_anon_6: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_6 {
                serverkey_bytes: srv_key.len() as u32,
            },
            serverkey_password: std::ptr::null(),
            serverkey_password_len: 0_u32,
            ..Default::default()
        });
        Self { cfg, alpn_proto }
    }
    fn get_cfg_ptr_mut(&mut self) -> *mut esp_tls_cfg_server {
        &mut *self.cfg as *mut _
    }
}

#[allow(dead_code)]
struct Esp32ClientConfig {
    cfg: Box<esp_tls_cfg>,
    alpn_proto: Vec<*const c_char>,
}

impl Esp32ClientConfig {
    fn new() -> Self {
        let mut alpn_proto = vec![ALPN_PROTOCOLS.as_ptr() as *const i8, std::ptr::null()];
        // TODO(RSDK-10200): Missing fields when using ESP-IDF 5
        let cfg = Box::new(esp_tls_cfg {
            alpn_protos: alpn_proto.as_mut_ptr(),
            __bindgen_anon_1: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_1 {
                cacert_buf: std::ptr::null(),
            },
            __bindgen_anon_2: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_2 {
                cacert_bytes: 0_u32,
            },
            __bindgen_anon_3: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_3 {
                clientcert_buf: std::ptr::null(),
            },
            __bindgen_anon_4: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_4 {
                clientcert_bytes: 0_u32,
            },
            __bindgen_anon_5: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_5 {
                clientkey_buf: std::ptr::null(),
            },
            __bindgen_anon_6: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_6 {
                clientkey_bytes: 0_u32,
            },
            clientkey_password: std::ptr::null(),
            clientkey_password_len: 0_u32,
            non_block: true,
            use_secure_element: false,
            use_global_ca_store: false,
            skip_common_name: false,
            keep_alive_cfg: std::ptr::null_mut(),
            crt_bundle_attach: Some(esp_crt_bundle_attach),
            ds_data: std::ptr::null_mut(),
            if_name: std::ptr::null_mut(),
            is_plain_tcp: false,
            timeout_ms: 50000,
            common_name: std::ptr::null(),
            ..Default::default()
        });
        Self { cfg, alpn_proto }
    }
    fn get_cfg_ptr(&self) -> *const esp_tls_cfg {
        &*self.cfg as *const _
    }
}

#[allow(dead_code)]
pub(crate) struct Esp32ServerTlsStream<IO> {
    cfg: Arc<Esp32ServerConfig>,
    io: IO,
}

impl<IO> AsyncRead for Esp32ServerTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}
impl<IO> AsyncWrite for Esp32ServerTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_close(cx)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }
}

struct Esp32Accept<IO> {
    cfg: Arc<Esp32ServerConfig>,
    state: TlsHandshake<IO>,
}

impl<IO> Esp32Accept<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + AsRawFd,
{
    fn new(stream: IO, mut cfg: Esp32ServerConfig) -> Result<Self, std::io::Error> {
        let tls_context = Esp32TLSContext::new()?;
        unsafe {
            esp!(esp_tls_server_session_create(
                cfg.get_cfg_ptr_mut(),
                stream.as_raw_fd(),
                *tls_context
            ))
        }
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;

        let io = AsyncSSLStream::new(SSLContext::Esp32TLSContext(tls_context), stream).unwrap();
        Ok(Self {
            cfg: Arc::new(cfg),
            state: TlsHandshake::Handshake(io),
        })
    }
}

pub(crate) enum TlsHandshake<IO> {
    Handshake(AsyncSSLStream<IO>),
    HandshakeError(SSLError),
    End,
}

impl<IO> Future for TlsHandshake<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    type Output = Result<AsyncSSLStream<IO>, SSLError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut io = match mem::replace(this, TlsHandshake::End) {
            TlsHandshake::Handshake(io) => io,
            TlsHandshake::HandshakeError(err) => return Poll::Ready(Err(err)),
            TlsHandshake::End => panic!("invalid state during handshake"),
        };
        match Pin::new(&mut io).poll_accept(cx) {
            Poll::Pending => {
                *this = TlsHandshake::Handshake(io);
                return Poll::Pending;
            }
            Poll::Ready(Err(err)) => {
                *this = TlsHandshake::HandshakeError(err);
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            Poll::Ready(Ok(_)) => (),
        }
        Poll::Ready(Ok(io))
    }
}

impl<IO> Future for Esp32Accept<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + AsRawFd,
{
    type Output = Result<Esp32ServerTlsStream<AsyncSSLStream<IO>>, SSLError>;
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let r = ready!(Pin::new(&mut self.state).poll(cx));
        match r {
            Ok(io) => Poll::Ready(Ok(Esp32ServerTlsStream {
                io,
                cfg: self.cfg.clone(),
            })),
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}
#[allow(dead_code)]
pub(crate) struct Esp32ClientTlsStream<IO> {
    cfg: Arc<Esp32ClientConfig>,
    host: Arc<CString>,
    io: IO,
}
impl<IO> AsyncRead for Esp32ClientTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}
impl<IO> AsyncWrite for Esp32ClientTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_close(cx)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }
}
struct Esp32Connect<IO> {
    cfg: Arc<Esp32ClientConfig>,
    host: Arc<CString>,
    state: TlsHandshake<IO>,
}
impl<IO> Esp32Connect<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn new(stream: IO, cfg: Esp32ClientConfig, addr: &Uri) -> Result<Self, std::io::Error> {
        let tls_context = Esp32TLSContext::new()?;

        let host = CString::new(addr.host().unwrap()).unwrap();
        unsafe {
            esp!(esp_create_mbedtls_handle(
                host.as_ptr(),
                host.as_bytes().len() as i32,
                cfg.get_cfg_ptr() as *const c_void,
                *tls_context
            ))
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?
        };

        unsafe {
            mbedtls_ssl_conf_read_timeout(
                (*(esp_tls_get_ssl_context(*tls_context) as *mut mbedtls_ssl_context)).private_conf
                    as *mut mbedtls_ssl_config,
                30 * 1000,
            );
        }
        let io = AsyncSSLStream::new(SSLContext::Esp32TLSContext(tls_context), stream).unwrap();
        Ok(Self {
            cfg: Arc::new(cfg),
            state: TlsHandshake::Handshake(io),
            host: Arc::new(host),
        })
    }
}

impl<IO> Future for Esp32Connect<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + AsRawFd,
{
    type Output = Result<Esp32ClientTlsStream<AsyncSSLStream<IO>>, SSLError>;
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let r = ready!(Pin::new(&mut self.state).poll(cx));
        match r {
            Ok(io) => Poll::Ready(Ok(Esp32ClientTlsStream {
                io,
                cfg: self.cfg.clone(),
                host: self.host.clone(),
            })),
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

pub(crate) enum Esp32TlsStream<IO> {
    Client(Esp32ClientTlsStream<IO>),
    Server(Esp32ServerTlsStream<IO>),
}

impl<IO> AsyncWrite for Esp32TlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Esp32TlsStream::Client(c) => Pin::new(c).poll_close(cx),
            Esp32TlsStream::Server(c) => Pin::new(c).poll_close(cx),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Esp32TlsStream::Client(c) => Pin::new(c).poll_flush(cx),
            Esp32TlsStream::Server(c) => Pin::new(c).poll_flush(cx),
        }
    }
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Esp32TlsStream::Client(c) => Pin::new(c).poll_write(cx, buf),
            Esp32TlsStream::Server(c) => Pin::new(c).poll_write(cx, buf),
        }
    }
}
impl<IO> AsyncRead for Esp32TlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Esp32TlsStream::Client(c) => Pin::new(c).poll_read(cx, buf),
            Esp32TlsStream::Server(c) => Pin::new(c).poll_read(cx, buf),
        }
    }
}

impl<IO> From<Esp32ServerTlsStream<IO>> for Esp32TlsStream<IO> {
    fn from(value: Esp32ServerTlsStream<IO>) -> Self {
        Self::Server(value)
    }
}

impl<IO> From<Esp32ClientTlsStream<IO>> for Esp32TlsStream<IO> {
    fn from(value: Esp32ClientTlsStream<IO>) -> Self {
        Self::Client(value)
    }
}

#[derive(Default)]
pub struct Esp32H2Connector {
    srv_cert: Option<CString>,
    srv_key: Option<CString>,
}

impl ViamH2Connector for Esp32H2Connector {
    fn set_server_certificates(&mut self, srv_cert: Vec<u8>, srv_key: Vec<u8>) {
        let _ = self.srv_cert.replace(CString::new(srv_cert).unwrap());
        let _ = self.srv_key.replace(CString::new(srv_key).unwrap());
    }
    fn accept_connection(
        &self,
        connection: Async<TcpStream>,
    ) -> Result<std::pin::Pin<Box<dyn IntoHttp2Stream>>, std::io::Error> {
        if self.srv_cert.is_some() && self.srv_key.is_some() {
            let cfg = Esp32ServerConfig::new(
                self.srv_cert.as_ref().unwrap().to_bytes_with_nul(),
                self.srv_key.as_ref().unwrap().to_bytes_with_nul(),
            );
            let conn = Esp32Accept::new(connection, cfg)?;
            Ok(Box::pin(Esp32StreamAcceptor(conn)))
        } else {
            Ok(Box::pin(Esp32StreamInsecureAcceptor(Some(connection))))
        }
    }
    fn connect_to(
        &self,
        uri: &hyper::Uri,
    ) -> Result<std::pin::Pin<Box<dyn IntoHttp2Stream>>, std::io::Error> {
        if uri.scheme_str().is_some_and(|s| s == "http") {
            log::info!("insecurely connecting to {:?}", uri);
            let stream =
                async_io::Async::new(TcpStream::connect(uri.authority().unwrap().as_str())?)
                    .unwrap();
            return Ok(Box::pin(Esp32StreamInsecureAcceptor(Some(stream))));
        }
        let stream = Async::new(TcpStream::connect(uri.authority().unwrap().as_str())?).unwrap();
        let cfg = Esp32ClientConfig::new();
        let conn = Esp32Connect::new(stream, cfg, uri)?;
        Ok(Box::pin(Esp32StreamConnector(conn)))
    }
}

pub struct Esp32StreamInsecureAcceptor(Option<Async<TcpStream>>);
impl IntoHttp2Stream for Esp32StreamInsecureAcceptor {}

impl Future for Esp32StreamInsecureAcceptor {
    type Output = Result<Box<dyn HTTP2Stream>, std::io::Error>;
    fn poll(mut self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(Ok(Box::new(Esp32Stream::LocalPlain(
            self.0.take().unwrap(),
        ))))
    }
}

pub struct Esp32StreamConnector(Esp32Connect<Async<TcpStream>>);
impl IntoHttp2Stream for Esp32StreamConnector {}
impl Future for Esp32StreamConnector {
    type Output = Result<Box<dyn HTTP2Stream>, std::io::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result: Self::Output = futures_lite::ready!(self.0.poll(cx))
            .map(|r| Box::new(Esp32Stream::TLSStream(r.into())) as Box<dyn HTTP2Stream>)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err));

        Poll::Ready(result)
    }
}

pub struct Esp32StreamAcceptor(Esp32Accept<Async<TcpStream>>);
impl IntoHttp2Stream for Esp32StreamAcceptor {}
impl Future for Esp32StreamAcceptor {
    type Output = Result<Box<dyn HTTP2Stream>, std::io::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result: Self::Output = futures_lite::ready!(self.0.poll(cx))
            .map(|r| Box::new(Esp32Stream::TLSStream(r.into())) as Box<dyn HTTP2Stream>)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err));

        Poll::Ready(result)
    }
}

/// Enum to represent a TCP stream (either plain or encrypted)
pub(crate) enum Esp32Stream {
    LocalPlain(Async<TcpStream>),
    TLSStream(Esp32TlsStream<AsyncSSLStream<Async<TcpStream>>>),
}

impl rt::Read for Esp32Stream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let uninit_buf = unsafe { &mut *(buf.as_mut() as *mut [MaybeUninit<u8>] as *mut [u8]) };
        match &mut *self {
            Esp32Stream::LocalPlain(s) => {
                futures_lite::pin!(s);
                match ready!(s.poll_read(cx, uninit_buf)) {
                    Ok(s) => {
                        unsafe { buf.advance(s) };
                        Poll::Ready(Ok(()))
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Esp32Stream::TLSStream(s) => {
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

impl rt::Write for Esp32Stream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match &mut *self {
            Esp32Stream::LocalPlain(s) => {
                futures_lite::pin!(s);

                match ready!(s.poll_write(cx, buf)) {
                    Ok(s) => Poll::Ready(Ok(s)),
                    Err(_) => Poll::Pending,
                }
            }
            Esp32Stream::TLSStream(s) => {
                futures_lite::pin!(s);

                match ready!(s.poll_write(cx, buf)) {
                    Ok(s) => Poll::Ready(Ok(s)),
                    Err(_) => Poll::Pending,
                }
            }
        }
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match &mut *self {
            Esp32Stream::LocalPlain(s) => {
                futures_lite::pin!(s);
                s.poll_flush(cx)
            }
            Esp32Stream::TLSStream(s) => {
                futures_lite::pin!(s);
                s.poll_flush(cx)
            }
        }
    }
    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match &mut *self {
            Esp32Stream::LocalPlain(s) => {
                futures_lite::pin!(s);
                s.poll_close(cx)
            }
            Esp32Stream::TLSStream(s) => {
                futures_lite::pin!(s);
                s.poll_close(cx)
            }
        }
    }
}
