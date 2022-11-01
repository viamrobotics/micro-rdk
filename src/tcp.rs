use futures_lite::io;

use log::*;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::io::{Read, Write};
use std::os::unix::prelude::AsRawFd;
use std::{
    marker::PhantomData,
    net::{Shutdown, TcpListener, TcpStream},
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::tls::{ESP32TLSStream, Esp32tls};

pub struct Esp32Listener {
    listener: TcpListener,
    #[allow(dead_code)]
    addr: SockAddr,
    _marker: PhantomData<*const ()>,
    tls: Option<Box<Esp32tls>>,
}

impl Esp32Listener {
    pub fn new(addr: SockAddr, tls: Option<Box<Esp32tls>>) -> anyhow::Result<Self> {
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
    pub fn accept(&mut self) -> anyhow::Result<Esp32Stream> {
        let (conn, _) = self.listener.accept()?;
        conn.set_nonblocking(true).expect("cannot set nodelay");
        match &mut self.tls {
            Some(tls) => {
                info!("opening TLS ctx");
                let stream = tls.open_ssl_context(conn)?;
                info!("handshake down");
                return Ok(Esp32Stream::TLSStream(Box::new(stream)));
            }
            None => {}
        }
        Ok(Esp32Stream::LocalPlain(conn))
    }
}

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
#[derive(Debug)]
pub enum Esp32Stream {
    LocalPlain(TcpStream),
    TLSStream(Box<ESP32TLSStream>),
}

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

impl AsyncWrite for Esp32Stream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Esp32Stream::LocalPlain(s) => match s.write(buf) {
                Ok(s) => {
                    //info!("DW");
                    Poll::Ready(Ok(s))
                }
                Err(_) => Poll::Pending,
            },
            Esp32Stream::TLSStream(s) => match s.write(buf) {
                Ok(s) => {
                    //info!("DW");
                    Poll::Ready(Ok(s))
                }
                Err(_) => Poll::Pending,
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
