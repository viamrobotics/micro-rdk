use std::{
    io::Result,
    net::{SocketAddr, UdpSocket},
    ops::{Index, IndexMut},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use async_io::{Async, Readable};

use futures_lite::{ready, AsyncRead, AsyncWrite, Future, FutureExt};

#[derive(Clone, Copy, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum MuxDirection {
    DTLS,
    STUN,
    // This is the default value it's a placeholder so we panic if for some reason we try
    // to index with this.
    //TODO remove once testing is done
    NODIR,
}

impl Index<MuxDirection> for [MuxState] {
    type Output = MuxState;
    fn index(&self, index: MuxDirection) -> &Self::Output {
        match index {
            MuxDirection::DTLS => &self[0],
            MuxDirection::STUN => &self[1],
            MuxDirection::NODIR => panic!(),
        }
    }
}
impl IndexMut<MuxDirection> for [MuxState] {
    fn index_mut(&mut self, index: MuxDirection) -> &mut Self::Output {
        match index {
            MuxDirection::DTLS => &mut self[0],
            MuxDirection::STUN => &mut self[1],
            MuxDirection::NODIR => panic!(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct UdpMuxer {
    socket: Arc<Async<UdpSocket>>,
    mux: Arc<Mutex<[MuxState; 2]>>,
}

impl Drop for UdpMuxer {
    fn drop(&mut self) {
        if Arc::strong_count(&self.mux) == 1 {
            log::debug!("dropping muxer");
        }
    }
}

struct UdpMuxReadable<'a> {
    muxer: &'a UdpMuxer,
    dir: MuxDirection,
    readable: Readable<'a, UdpSocket>,
    ran_once: bool,
}

impl Future for UdpMuxReadable<'_> {
    type Output = Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            muxer,
            dir,
            readable,
            ran_once,
        } = &mut *self;

        if *ran_once {
            return Poll::Ready(Ok(()));
        }
        *ran_once = true;

        muxer.register_waker(cx.waker().clone(), *dir);
        readable.poll(cx)
    }
}

impl Drop for UdpMuxReadable<'_> {
    fn drop(&mut self) {
        let state = &mut self.muxer.mux.lock().unwrap()[self.dir];
        let _ = state.waker.take();
    }
}

impl UdpMuxer {
    fn readable_udp_muxer(&self, dir: MuxDirection) -> UdpMuxReadable<'_> {
        UdpMuxReadable {
            muxer: self,
            dir,
            readable: self.socket.readable(),
            ran_once: false,
        }
    }
    pub(crate) fn new(socket: Arc<Async<UdpSocket>>) -> Self {
        Self {
            socket: socket.clone(),
            mux: Default::default(),
        }
    }
    pub(crate) fn get_stun_mux(&self) -> Option<UdpMux> {
        let state = &mut self.mux.lock().unwrap()[MuxDirection::STUN];
        if !state.is_listening {
            state.is_listening = true;
            Some(UdpMux {
                muxer: self.clone(),
                direction: MuxDirection::STUN,
                peer_addr: None,
            })
        } else {
            None
        }
    }
    pub(crate) fn get_dtls_mux(&self) -> Option<UdpMux> {
        let state = &mut self.mux.lock().unwrap()[MuxDirection::DTLS];
        if !state.is_listening {
            state.is_listening = true;
            Some(UdpMux {
                muxer: self.clone(),
                direction: MuxDirection::DTLS,
                peer_addr: None,
            })
        } else {
            None
        }
    }
    async fn recv_from(&self, dir: MuxDirection, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        loop {
            let r = match self.peek() {
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (0, MuxDirection::NODIR),
                Ok(s) => s,
                Err(e) => return Err(e),
            };
            if r.0 != 0 {
                if dir == r.1 {
                    let socket = self.socket.as_ref().get_ref();
                    return socket.recv_from(buf);
                }
                if self.yield_or_discard(r.1, r.0)? {
                    continue;
                }
            }
            self.readable_udp_muxer(dir).await?;
        }
    }

    fn register_waker(&self, waker: Waker, dir: MuxDirection) {
        let mux = &mut self.mux.lock().unwrap()[dir];
        if let Some(w) = mux.waker.take() {
            if w.will_wake(&waker) {
                mux.waker = Some(w);
                return;
            }
            w.wake();
        }
        mux.waker = Some(waker);
    }
    fn deregister_waker(&self, dir: MuxDirection) {
        let mux = &mut self.mux.lock().unwrap()[dir];
        let _ = mux.waker.take();
    }

    fn yield_or_discard(&self, dir: MuxDirection, _len: u16) -> Result<bool> {
        let socket = self.socket.as_ref().get_ref();
        let mux = &mut self.mux.lock().unwrap()[dir];
        if !mux.is_listening {
            let mut buf = [0_u8; 1];
            let _ = socket.recv_from(&mut buf)?;
            return Ok(true);
        }
        if let Some(w) = mux.waker.take() {
            w.wake();
            return Ok(false);
        }
        Ok(false)
    }

    // will peek at the next available message on the socket
    // if it's size is less than the minimum header size the packet is discarded
    // otherwise the type and length will be returned
    fn peek(&self) -> Result<(u16, MuxDirection)> {
        let socket = self.socket.as_ref().get_ref();
        let mut buf = [0_u8; 13];
        let (len, _) = socket.peek_from(&mut buf)?;
        if len != 13 {
            let _ = socket.recv_from(&mut buf)?;
            return Ok((0, MuxDirection::NODIR));
        }
        Ok(self.read_header(buf))
    }

    fn read_header(&self, hdr: [u8; 13]) -> (u16, MuxDirection) {
        let msg_type = hdr[0];
        if msg_type < 2 {
            // stun message
            let len: u16 = u16::from_be_bytes(hdr[2..4].try_into().unwrap());
            (len, MuxDirection::STUN)
        } else {
            // assume DTLS record
            let len: u16 = u16::from_be_bytes(hdr[11..13].try_into().unwrap());
            (len, MuxDirection::DTLS)
        }
    }
    async fn send_to(&self, buf: &[u8], peer: SocketAddr) -> Result<usize> {
        loop {
            let socket = self.socket.as_ref().get_ref();
            match socket.send_to(buf, peer) {
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::OutOfMemory => {}
                Ok(s) => return Ok(s),
                Err(e) => return Err(e),
            }
            self.socket.writable().await?;
        }
    }

    fn poll_recv_from(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        dir: MuxDirection,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, SocketAddr)>> {
        self.register_waker(cx.waker().clone(), dir);
        loop {
            let r = match self.peek() {
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (0, MuxDirection::NODIR),
                Ok(s) => s,
                Err(e) => return Poll::Ready(Err(e)),
            };
            if r.0 != 0 {
                if dir == r.1 {
                    let socket = self.socket.as_ref().get_ref();
                    self.deregister_waker(dir);
                    return Poll::Ready(socket.recv_from(buf));
                }

                match self.yield_or_discard(r.1, r.0) {
                    Err(e) => return Poll::Ready(Err(e)),
                    Ok(s) => {
                        if s {
                            continue;
                        }
                    }
                }
            }
            let _ = ready!(self.socket.poll_readable(cx));
        }
    }
    fn poll_send_to(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        peer: SocketAddr,
    ) -> Poll<Result<usize>> {
        loop {
            let socket = self.socket.as_ref().get_ref();
            match socket.send_to(buf, peer) {
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::OutOfMemory => {}

                Ok(s) => return Poll::Ready(Ok(s)),
                Err(e) => return Poll::Ready(Err(e)),
            }
            let _ = ready!(self.socket.poll_writable(cx));
        }
    }
}

#[derive(Default)]
struct MuxState {
    waker: Option<Waker>, // waker is present if a consumer has yield because it's waiting it's turn on the socket
    is_listening: bool,   // whether there is a consumer listening
}

pub struct UdpMux {
    muxer: UdpMuxer,
    direction: MuxDirection, // symbolize the interest a consumer has on a particular message type
    peer_addr: Option<SocketAddr>,
}

impl UdpMux {
    pub(crate) async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        self.muxer.recv_from(self.direction, buf).await
    }
    // TODO consider SocketAddrV4
    pub(crate) async fn send_to(&self, buf: &[u8], peer: SocketAddr) -> Result<usize> {
        self.muxer.send_to(buf, peer).await
    }

    pub(crate) fn local_address(&self) -> Result<SocketAddr> {
        self.muxer.socket.get_ref().local_addr()
    }
}

impl Drop for UdpMux {
    fn drop(&mut self) {
        let state = &mut self.muxer.mux.lock().unwrap()[self.direction];
        state.is_listening = false;
        let _ = state.waker.take();
    }
}

impl AsyncRead for UdpMux {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let direction = self.direction;
        let r = ready!(Pin::new(&mut self.muxer).poll_recv_from(cx, direction, buf));
        match r {
            Ok((len, peer_addr)) => {
                let _ = self.peer_addr.insert(peer_addr);
                Poll::Ready(Ok(len))
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncWrite for UdpMux {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        if let Some(peer_addr) = self.peer_addr {
            Pin::new(&mut self.muxer).poll_send_to(cx, buf, peer_addr)
        } else {
            Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no peer set",
            )))
        }
    }
    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {

    use std::time::Duration;
    use std::{net::UdpSocket, sync::Arc};

    use async_io::{Async, Timer};
    use bytes::{BufMut, Bytes, BytesMut};
    //use futures_lite::FutureExt;
    use futures_lite::{AsyncReadExt, FutureExt as OtherFutureExt, StreamExt};
    use futures_util::FutureExt;
    use rand::Rng;

    use crate::common::webrtc::udp_mux::{MuxDirection, UdpMuxer};

    fn dtls_packet(len: u16, typ: u8) -> Bytes {
        let mut buf = BytesMut::with_capacity(len as usize + 13);
        let paylod = (0..len).map(|_| 0xAA).collect::<Vec<u8>>();
        buf.put_u8(typ);
        buf.put_slice(&[0xCC_u8; 10]);
        buf.put_slice(&len.to_be_bytes());
        buf.put_slice(&paylod);
        buf.freeze()
    }
    fn stun_packet(len: u16, typ: u8) -> Bytes {
        let mut buf = BytesMut::with_capacity(len as usize + 4);
        let paylod = (0..len).map(|_| 0xBB).collect::<Vec<u8>>();
        buf.put_u8(0);
        buf.put_u8(typ);
        buf.put_slice(&len.to_be_bytes());
        buf.put_slice(&paylod);
        buf.freeze()
    }

    #[test_log::test]
    fn test_drop() {
        let srv_socket = UdpSocket::bind("127.0.0.1:0");
        assert!(srv_socket.is_ok());
        let srv_socket = srv_socket.unwrap();
        let srv_socket = Async::new(srv_socket);
        assert!(srv_socket.is_ok());
        let srv_socket = Arc::new(srv_socket.unwrap());

        let muxer = UdpMuxer::new(srv_socket);
        {
            let dtls = muxer.get_dtls_mux();
            assert!(dtls.is_some());
            let other_dtls = muxer.get_dtls_mux();
            assert!(other_dtls.is_none());
            let dtls_mux = muxer.mux.lock().unwrap();
            assert!(dtls_mux[MuxDirection::DTLS].is_listening);
        }
        {
            let dtls_mux = muxer.mux.lock().unwrap();
            assert!(!dtls_mux[MuxDirection::DTLS].is_listening);
        }
        {
            let stun = muxer.get_stun_mux();
            assert!(stun.is_some());
            let other_stun = muxer.get_stun_mux();
            assert!(other_stun.is_none());
            let stun_mux = muxer.mux.lock().unwrap();
            assert!(stun_mux[MuxDirection::STUN].is_listening);
        }
        {
            let stun_mux = muxer.mux.lock().unwrap();
            assert!(!stun_mux[MuxDirection::STUN].is_listening);
        }
    }

    #[test_log::test]
    fn later_drop_interest() {
        let local_ex = async_executor::LocalExecutor::new();

        let srv_socket = UdpSocket::bind("127.0.0.1:0");
        assert!(srv_socket.is_ok());
        let srv_socket = srv_socket.unwrap();

        let addr = srv_socket.local_addr();
        assert!(addr.is_ok());
        let addr = addr.unwrap();

        let srv_socket = Async::new(srv_socket);
        assert!(srv_socket.is_ok());
        let srv_socket = Arc::new(srv_socket.unwrap());
        let muxer = UdpMuxer::new(srv_socket);

        let dtls = muxer.get_dtls_mux();
        assert!(dtls.is_some());
        let mut dtls = dtls.unwrap();

        let (c_dtls_tx, c_dtls_rx) = async_channel::unbounded::<Bytes>();
        let (msg_read_tx, msg_read_rx) = async_channel::unbounded::<()>();

        let read_dtls = async move {
            loop {
                let mut buf = [0_u8; 1500];
                let read = dtls.read(&mut buf).await;
                assert!(read.is_ok());
                let read = read.unwrap();
                let real_bytes = c_dtls_rx.try_recv();
                assert!(real_bytes.is_ok());
                let real_bytes = real_bytes.unwrap();
                assert_eq!(real_bytes.len(), read);
                let compare = real_bytes.iter().zip(buf.iter()).all(|(a, b)| a == b);
                assert!(compare);
                assert!(msg_read_tx.try_send(()).is_ok());
            }
        };

        let client = async move {
            Timer::after(Duration::from_millis(100)).await;

            let client_socket = UdpSocket::bind("127.0.0.1:0");
            assert!(client_socket.is_ok());
            let client_socket = client_socket.unwrap();

            let client_socket = Async::new(client_socket);
            assert!(client_socket.is_ok());
            let client_socket = client_socket.unwrap();

            let dtls_msg_interleaved = 10;
            {
                // interleave stun and dtls packets starting with stun
                let stun = muxer.get_stun_mux();
                assert!(stun.is_some());
                for _ in 0..dtls_msg_interleaved {
                    let stun_pkt = stun_packet(rand::rng().random_range(35..250), 2);
                    let r = client_socket.send_to(&stun_pkt, addr).await;
                    assert!(r.is_ok());

                    let dtls_pkt = dtls_packet(rand::rng().random_range(35..800), 23);
                    assert!(c_dtls_tx.try_send(dtls_pkt.clone()).is_ok());
                    let r = client_socket.send_to(&dtls_pkt, addr).await;
                    assert!(r.is_ok());
                }

                // pipeline is stalling
                let fut = async {
                    let r = msg_read_rx.clone().recv().into_stream().count().await;
                    Some(r)
                }
                .or(async {
                    Timer::after(Duration::from_millis(500)).await;
                    None
                })
                .await;
                assert!(fut.is_none());
            }
            // pipeline not stalling
            let fut = async {
                let mut n = 0;
                while n != dtls_msg_interleaved {
                    let r = msg_read_rx.recv().await;
                    assert!(r.is_ok());
                    n += 1;
                }
                Some(n)
            }
            .or(async {
                Timer::after(Duration::from_millis(500)).await;
                None
            })
            .await;
            assert!(fut.is_some());
            assert_eq!(fut.unwrap(), dtls_msg_interleaved);
        };
        local_ex.spawn(read_dtls).detach();
        futures_lite::future::block_on(local_ex.run(client));
    }

    #[test_log::test]
    fn test_interleave() {
        let local_ex = async_executor::LocalExecutor::new();

        let srv_socket = UdpSocket::bind("127.0.0.1:0");
        assert!(srv_socket.is_ok());
        let srv_socket = srv_socket.unwrap();

        let addr = srv_socket.local_addr();
        assert!(addr.is_ok());
        let addr = addr.unwrap();

        let srv_socket = Async::new(srv_socket);
        assert!(srv_socket.is_ok());
        let srv_socket = Arc::new(srv_socket.unwrap());

        let muxer = UdpMuxer::new(srv_socket);

        let dtls = muxer.get_dtls_mux();
        assert!(dtls.is_some());
        let mut dtls = dtls.unwrap();
        let stun = muxer.get_stun_mux();
        assert!(stun.is_some());
        let stun = stun.unwrap();
        let (c_stun_tx, c_stun_rx) = async_channel::bounded::<Bytes>(1);
        let (c_dtls_tx, c_dtls_rx) = async_channel::bounded::<Bytes>(1);

        let read_dtls = async move {
            loop {
                let mut buf = [0_u8; 1500];
                let read = dtls.read(&mut buf).await;
                assert!(read.is_ok());
                let read = read.unwrap();
                let real_bytes = c_dtls_rx.try_recv();
                assert!(real_bytes.is_ok());
                let real_bytes = real_bytes.unwrap();
                assert_eq!(real_bytes.len(), read);
                let compare = real_bytes.iter().zip(buf.iter()).all(|(a, b)| a == b);
                assert!(compare);
            }
        };

        let read_stun = async move {
            loop {
                let mut buf = [0_u8; 1500];
                let read = stun.recv_from(&mut buf).await;
                assert!(read.is_ok());
                let read = read.unwrap();
                let real_bytes = c_stun_rx.try_recv();
                assert!(real_bytes.is_ok());
                let real_bytes = real_bytes.unwrap();
                assert_eq!(real_bytes.len(), read.0);
                let compare = real_bytes.iter().zip(buf.iter()).all(|(a, b)| a == b);
                assert!(compare);
            }
        };

        let client = async move {
            Timer::after(Duration::from_millis(500)).await;
            let client_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            let stun_pkt = stun_packet(20, 2);
            let dtls_pkt = dtls_packet(389, 23);

            let client_socket = Async::new(client_socket).unwrap();
            assert!(c_stun_tx.send(stun_pkt.clone()).await.is_ok());
            assert!(client_socket.send_to(&stun_pkt, addr).await.is_ok());

            assert!(c_dtls_tx.send(dtls_pkt.clone()).await.is_ok());
            assert!(client_socket.send_to(&dtls_pkt, addr).await.is_ok());

            Timer::after(Duration::from_millis(500)).await;

            let dtls_pkt = dtls_packet(160, 20);
            assert!(c_dtls_tx.send(dtls_pkt.clone()).await.is_ok());
            assert!(client_socket.send_to(&dtls_pkt, addr).await.is_ok());

            let dtls_pkt = dtls_packet(780, 21);
            assert!(c_dtls_tx.send(dtls_pkt.clone()).await.is_ok());
            assert!(client_socket.send_to(&dtls_pkt, addr).await.is_ok());

            Timer::after(Duration::from_millis(500)).await;
        };

        local_ex.spawn(read_dtls).detach();
        local_ex.spawn(read_stun).detach();
        futures_lite::future::block_on(local_ex.run(client));
    }
}
