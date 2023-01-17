use crate::native::tls::{NativeTls, NativeTlsStream};
use futures_lite::io;
use log::*;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::io::{Read, Write};
use std::{
    marker::PhantomData,
    net::{Shutdown, TcpListener, TcpStream},
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite};

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
    pub fn new(addr: SockAddr, tls: Option<Box<NativeTls>>) -> anyhow::Result<Self> {
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

    /// Accept the next incoming connection. Will block until a connection is established
    pub fn accept(&mut self) -> anyhow::Result<NativeStream> {
        let (conn, _) = self.listener.accept()?;
        conn.set_nonblocking(true).expect("cannot set nodelay");
        let stream = match &mut self.tls {
            Some(tls) => {
                info!("opening TLS ctx");
                let stream = tls.open_ssl_context(Some(conn))?;
                info!("handshake done");
                NativeStream::TLSStream(Box::new(stream))
            }
            None => NativeStream::LocalPlain(conn),
        };
        Ok(stream)
    }
}

/// Trait helper for hyper based server
impl hyper::server::accept::Accept for NativeListener {
    type Conn = NativeStream;
    type Error = io::Error;
    fn poll_accept(
        self: std::pin::Pin<&mut Self>,
        _: &mut Context,
    ) -> std::task::Poll<Option<Result<Self::Conn, Self::Error>>> {
        let (stream, peer) = self.listener.accept()?;
        info!("Connected to {:?}", peer);
        stream.set_nonblocking(true).expect("cannot set nodelay");
        let stream = NativeStream::LocalPlain(stream);
        Poll::Ready(Some(Ok(stream)))
    }
}

/// Enum to represent a TCP stream (either plain or encrypted)
pub enum NativeStream {
    LocalPlain(TcpStream),
    TLSStream(Box<NativeTlsStream>),
}

/// Implement AsyncRead trait for NativeStream
impl AsyncRead for NativeStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            NativeStream::LocalPlain(s) => match s.read(buf.initialize_unfilled()) {
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
            NativeStream::TLSStream(s) => match s.read(buf.initialize_unfilled()) {
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

/// Implement AsyncWrite trait for NativeStream
impl AsyncWrite for NativeStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            NativeStream::LocalPlain(s) => match s.write(buf) {
                Ok(s) => Poll::Ready(Ok(s)),
                Err(_) => Poll::Pending,
            },
            NativeStream::TLSStream(s) => match s.write(buf) {
                Ok(s) => Poll::Ready(Ok(s)),
                Err(_) => Poll::Pending,
            },
        }
    }

    fn poll_flush(mut self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            NativeStream::LocalPlain(s) => Poll::Ready(s.flush()),
            NativeStream::TLSStream(s) => Poll::Ready(s.flush()),
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            NativeStream::LocalPlain(s) => {
                s.shutdown(Shutdown::Write)?;
                Poll::Ready(Ok(()))
            }
            NativeStream::TLSStream(_) => Poll::Ready(Ok(())),
        }
    }
}
