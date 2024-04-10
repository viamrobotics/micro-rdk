use crate::common::conn::server::{AsyncableTcpListener, Http2Connector, OwnedListener};
use crate::native::tls::{NativeTls, NativeTlsStream};
use async_io::Async;
use futures_lite::future::FutureExt;

use futures_lite::{ready, Future};
use hyper::rt;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::fmt::Debug;
use std::io::{self};
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::pin::Pin;

use std::{
    marker::PhantomData,
    net::{TcpListener, TcpStream},
    task::{Context, Poll},
};

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

impl AsyncableTcpListener<NativeStream> for NativeListener {
    type Output = NativeTlsConnector;
    fn as_async_listener(&self) -> OwnedListener<Self::Output> {
        let nat = Async::new(self.listener.try_clone().unwrap()).unwrap();
        let inner = NativeIncoming {
            inner: Box::pin(async move { nat.accept().await }),
            tls: self.tls.clone(),
        };
        OwnedListener {
            inner: Box::pin(inner),
        }
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
        }
    }
}
