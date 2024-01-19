use crate::common::conn::server::{AsyncableTcpListener, Http2Connector, OwnedListener};
use crate::esp32::tls::{Esp32Tls, Esp32TlsStream};
use futures_lite::{io, Future};
use log::*;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::fmt::Debug;
use std::io::{Read, Write};
use std::sync::Arc;
use std::{
    marker::PhantomData,
    net::{Shutdown, TcpListener, TcpStream},
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite};

/// Struct to listen for incoming TCP connections
pub struct Esp32Listener {
    listener: Arc<TcpListener>,
    #[allow(dead_code)]
    addr: SockAddr,
    _marker: PhantomData<*const ()>,
    tls: Option<Box<Esp32Tls>>,
}

impl Esp32Listener {
    /// Creates a new Tcplistener
    pub fn new(addr: SockAddr, tls: Option<Box<Esp32Tls>>) -> anyhow::Result<Self> {
        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
        socket.set_reuse_address(true)?;
        socket.set_nodelay(true)?;
        socket.bind(&addr)?;
        socket.listen(128)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            listener: Arc::new(socket.into()),
            addr,
            _marker: PhantomData,
            tls,
        })
    }

    /// Accept the next incoming connection. Will block until a connection is established
    pub fn accept(&mut self) -> anyhow::Result<Esp32Stream> {
        let (conn, _) = self.listener.accept()?;
        conn.set_nonblocking(true)
            .expect("cannot set tcp port in non-blocking mode");
        let stream = match &mut self.tls {
            Some(tls) => {
                info!("opening TLS ctx");
                let stream = tls.open_ssl_context(Some(conn))?;
                info!("handshake done");
                Esp32Stream::TLSStream(Box::new(stream))
            }
            None => Esp32Stream::LocalPlain(conn),
        };
        Ok(stream)
    }
}

pub struct Esp32AsyncListener {
    inner: Arc<TcpListener>,
    tls: Option<Box<Esp32Tls>>,
}

impl Future for Esp32AsyncListener {
    type Output = std::io::Result<Esp32TlsConnector>;
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let stream = self.inner.accept();
        match stream {
            Ok(s) => {
                let s = Esp32TlsConnector {
                    tls: self.tls.clone(),
                    inner: Some(s.0),
                };
                Poll::Ready(Ok(s))
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

pub struct Esp32TlsConnector {
    tls: Option<Box<Esp32Tls>>,
    inner: Option<TcpStream>,
}

impl Debug for Esp32TlsConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Esp32TlsConnector")
            .field("inner", &self.inner)
            .field("tls", &self.tls.is_some())
            .finish()
    }
}

impl Http2Connector for Esp32TlsConnector {
    type Stream = Esp32Stream;
    fn accept(&mut self) -> std::io::Result<Self::Stream> {
        self.inner.as_ref().unwrap().set_nonblocking(true).unwrap();
        match &mut self.tls {
            Some(tls) => tls
                .open_ssl_context(Some(self.inner.take().unwrap()))
                .map(|s| Esp32Stream::TLSStream(Box::new(s)))
                .map_err(|e| std::io::Error::new(io::ErrorKind::Other, e)),
            None => Ok(Esp32Stream::LocalPlain(self.inner.take().unwrap())),
        }
    }
}

impl AsyncableTcpListener<Esp32Stream> for Esp32Listener {
    type Output = Esp32TlsConnector;
    fn as_async_listener(&self) -> OwnedListener<Self::Output> {
        let listener = Esp32AsyncListener {
            inner: self.listener.clone(),
            tls: self.tls.clone(),
        };
        OwnedListener {
            inner: Box::pin(listener),
        }
    }
}

/// Trait helper for hyper based server
impl hyper::server::accept::Accept for Esp32Listener {
    type Conn = Esp32Stream;
    type Error = io::Error;
    fn poll_accept(
        self: std::pin::Pin<&mut Self>,
        _: &mut Context,
    ) -> std::task::Poll<Option<Result<Self::Conn, Self::Error>>> {
        let (stream, peer) = self.listener.accept()?;
        info!("Connected to {:?}", peer);
        stream.set_nonblocking(true).expect("cannot set nodelay");
        let stream = Esp32Stream::LocalPlain(stream);
        Poll::Ready(Some(Ok(stream)))
    }
}

/// Enum to represent a TCP stream (either plain or encrypted)
#[derive(Debug)]
pub enum Esp32Stream {
    LocalPlain(TcpStream),
    TLSStream(Box<Esp32TlsStream>),
}

/// Implement AsyncRead trait for Esp32Stream
impl AsyncRead for Esp32Stream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Esp32Stream::LocalPlain(s) => match s.read(buf.initialize_unfilled()) {
                Ok(s) => {
                    buf.advance(s);
                    Poll::Ready(Ok(()))
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Err(e) => Poll::Ready(Err(e)),
            },
            Esp32Stream::TLSStream(s) => match s.read(buf.initialize_unfilled()) {
                Ok(s) => {
                    buf.advance(s);
                    Poll::Ready(Ok(()))
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Err(e) => Poll::Ready(Err(e)),
            },
        }
    }
}

/// Implement AsyncWrite trait for Esp32Stream
impl AsyncWrite for Esp32Stream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Esp32Stream::LocalPlain(s) => match s.write(buf) {
                Ok(s) => Poll::Ready(Ok(s)),
                Err(e) => Poll::Ready(Err(e)),
            },
            Esp32Stream::TLSStream(s) => match s.write(buf) {
                Ok(s) => Poll::Ready(Ok(s)),
                Err(e) => Poll::Ready(Err(e)),
            },
        }
    }

    fn poll_flush(mut self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Esp32Stream::LocalPlain(s) => Poll::Ready(s.flush()),
            Esp32Stream::TLSStream(s) => Poll::Ready(s.flush()),
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Esp32Stream::LocalPlain(s) => {
                s.shutdown(Shutdown::Write)?;
                Poll::Ready(Ok(()))
            }
            Esp32Stream::TLSStream(_) => Poll::Ready(Ok(())),
        }
    }
}
