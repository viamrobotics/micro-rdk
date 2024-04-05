use crate::common::conn::server::{AsyncableTcpListener, Http2Connector, OwnedListener};
use crate::esp32::tls::{Esp32TLS, Esp32TLSStream};
use async_io::Async;
use futures_lite::{io, ready, AsyncRead, AsyncWrite, Future};
use hyper::rt;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::fmt::Debug;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::{
    marker::PhantomData,
    net::{TcpListener, TcpStream},
    task::{Context, Poll},
};

/// Struct to listen for incoming TCP connections
pub struct Esp32Listener {
    listener: Arc<TcpListener>,
    #[allow(dead_code)]
    addr: SockAddr,
    _marker: PhantomData<*const ()>,
    tls: Option<Box<Esp32TLS>>,
}

impl Esp32Listener {
    /// Creates a new Tcplistener
    pub fn new(addr: SockAddr, tls: Option<Box<Esp32TLS>>) -> Result<Self, std::io::Error> {
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
    pub fn accept(&mut self) -> Result<Esp32Stream, std::io::Error> {
        let (conn, _) = self.listener.accept()?;
        conn.set_nonblocking(true)?;
        let stream = match &mut self.tls {
            Some(tls) => {
                let stream = tls.open_ssl_context(Some(Async::new(conn).unwrap()))?;
                Esp32Stream::TLSStream(Box::new(stream))
            }
            None => Esp32Stream::LocalPlain(Async::new(conn).unwrap()),
        };
        Ok(stream)
    }
}

pub struct Esp32AsyncListener {
    inner: Arc<TcpListener>,
    tls: Option<Box<Esp32TLS>>,
}

impl Future for Esp32AsyncListener {
    type Output = std::io::Result<Esp32TLSConnector>;
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let stream = self.inner.accept();
        match stream {
            Ok(s) => {
                let s = Esp32TLSConnector {
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

pub struct Esp32TLSConnector {
    tls: Option<Box<Esp32TLS>>,
    inner: Option<TcpStream>,
}

impl Debug for Esp32TLSConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Esp32TLSConnector")
            .field("inner", &self.inner)
            .field("tls", &self.tls.is_some())
            .finish()
    }
}

impl Http2Connector for Esp32TLSConnector {
    type Stream = Esp32Stream;
    async fn accept(&mut self) -> std::io::Result<Self::Stream> {
        self.inner.as_ref().unwrap().set_nonblocking(true).unwrap();
        match &mut self.tls {
            Some(tls) => tls
                .open_ssl_context(Some(Async::new(self.inner.take().unwrap()).unwrap()))
                .map(|s| Esp32Stream::TLSStream(Box::new(s)))
                .map_err(|e| std::io::Error::new(io::ErrorKind::Other, e)),
            None => Ok(Esp32Stream::LocalPlain(
                Async::new(self.inner.take().unwrap()).unwrap(),
            )),
        }
    }
}

impl AsyncableTcpListener<Esp32Stream> for Esp32Listener {
    type Output = Esp32TLSConnector;
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

/// Enum to represent a TCP stream (either plain or encrypted)
#[derive(Debug)]
pub enum Esp32Stream {
    LocalPlain(Async<TcpStream>),
    TLSStream(Box<Esp32TLSStream>),
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

/// Implement AsyncRead trait for Esp32Stream
impl tokio::io::AsyncRead for Esp32Stream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Esp32Stream::LocalPlain(s) => {
                futures_lite::pin!(s);
                match ready!(s.poll_read(cx, buf.initialize_unfilled())) {
                    Ok(s) => {
                        buf.advance(s);
                        Poll::Ready(Ok(()))
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Esp32Stream::TLSStream(s) => {
                futures_lite::pin!(s);
                match ready!(s.poll_read(cx, buf.initialize_unfilled())) {
                    Ok(s) => {
                        buf.advance(s);
                        Poll::Ready(Ok(()))
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
        }
    }
}

/// Implement AsyncWrite trait for Esp32Stream
impl tokio::io::AsyncWrite for Esp32Stream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
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
    ) -> Poll<io::Result<()>> {
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
    ) -> Poll<io::Result<()>> {
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
