#![allow(dead_code)]
use std::{
    fmt::Debug,
    io::Write,
    io::{ErrorKind, Read},
    net::{SocketAddr, SocketAddrV4},
    pin::Pin,
    task::{Poll, Waker},
};

use anyhow::bail;
use async_channel::{RecvError, SendError};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_lite::{future::Boxed, ready, AsyncRead, AsyncWrite, FutureExt};
use smol::net::UdpSocket;

#[derive(Debug)]
pub struct IoPkt {
    addr: SocketAddrV4,
    payload: Bytes,
}
unsafe impl Send for IoPktChannel {}
unsafe impl Sync for IoPktChannel {}

pub struct IoPktChannel {
    rx: smol::channel::Receiver<IoPkt>,
    transport_tx: smol::channel::Sender<IoPkt>,
    tx: smol::channel::Sender<IoPkt>,
    ip: Option<SocketAddrV4>,
    recv_operation: Option<Boxed<Result<IoPkt, RecvError>>>,
    send_operation: Option<Boxed<Result<(), SendError<IoPkt>>>>,
    waker: Option<Waker>,
}

impl Clone for IoPktChannel {
    fn clone(&self) -> Self {
        Self {
            rx: self.rx.clone(),
            transport_tx: self.transport_tx.clone(),
            tx: self.tx.clone(),
            ip: self.ip,
            recv_operation: None,
            send_operation: None,
            waker: None,
        }
    }
}

impl AsyncRead for IoPktChannel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        mut buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.recv_operation.is_none() {
            let cloned = self.rx.clone();
            let _ = self
                .recv_operation
                .replace(Box::pin(async move { cloned.recv().await }));
        }

        let result = match ready!(self.recv_operation.as_mut().unwrap().poll(cx)) {
            Ok(pkt) => {
                let _ = self.ip.insert(pkt.addr);
                if buf.len() < pkt.payload.len() {
                    // TODO(npm) remove the panic
                    panic!(
                        "expected buf len {} to be more than payload {}",
                        buf.len(),
                        pkt.payload.len()
                    );
                }
                let len = pkt.payload.len();
                buf.put(pkt.payload);
                Poll::Ready(Ok(len))
            }
            Err(e) => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e))),
        };
        let _ = self.recv_operation.take();
        result
    }
}

impl AsyncWrite for IoPktChannel {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let ip = if let Some(ip) = self.ip.as_ref() {
            *ip
        } else {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "missing ip",
            )));
        };
        let pkt = Bytes::copy_from_slice(buf);
        let pkt = IoPkt {
            addr: ip,
            payload: pkt,
        };
        if self.send_operation.is_none() {
            let cloned = self.transport_tx.clone();
            let _ = self
                .send_operation
                .replace(Box::pin(async move { cloned.send(pkt).await }));
        }
        let result = match ready!(self.send_operation.as_mut().unwrap().poll(cx)) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(e) => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e))),
        };
        let _ = self.send_operation.take();
        result
    }
    fn poll_close(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl Write for IoPktChannel {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let pkt = Bytes::copy_from_slice(buf);
        if let Some(ip) = self.ip.as_ref() {
            self.transport_tx
                .send_blocking(IoPkt {
                    addr: *ip,
                    payload: pkt,
                })
                .unwrap();
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Read for IoPktChannel {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len();

        if !buf.is_empty() {
            let pkt = match self.rx.try_recv() {
                Ok(pkt) => pkt,
                Err(e) => match e {
                    async_channel::TryRecvError::Empty => {
                        return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
                    }
                    _ => return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe)),
                },
            };
            let _ = self.ip.insert(pkt.addr);
            if buf.len() > pkt.payload.len() {
                buf.put(pkt.payload);
            } else {
                panic!(
                    "expected buf len {} to be more than payload {}",
                    buf.len(),
                    pkt.payload.len()
                );
            }
        }
        Ok(len - buf.len())
    }
}

impl Debug for IoPktChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IoPktChannel")
            .field("rx", &self.rx.len())
            .field("tx", &self.transport_tx.len())
            .finish()
    }
}

impl IoPktChannel {
    fn new(tx: smol::channel::Sender<IoPkt>) -> Self {
        let (txo, rx) = smol::channel::bounded::<IoPkt>(10);
        Self {
            rx,
            transport_tx: tx,
            tx: txo,
            ip: None,
            waker: None,
            recv_operation: None,
            send_operation: None,
        }
    }

    fn get_rx_channel(&self) -> anyhow::Result<smol::channel::Receiver<IoPkt>> {
        Ok(self.rx.clone())
    }
    fn get_tx_channel(&self) -> anyhow::Result<smol::channel::Sender<IoPkt>> {
        Ok(self.transport_tx.clone())
    }
    async fn send_pkt(&self, pkt: IoPkt) -> anyhow::Result<()> {
        if self.tx.is_full() {
            log::error!("packet was dropped");
            return Ok(());
        }
        self.tx
            .send(pkt)
            .await
            .map_err(|e| anyhow::anyhow!("error sending a pkt to a lower pipeline {:?}", e))
    }
    pub async fn send_to(&self, payload: Bytes, to: SocketAddrV4) -> anyhow::Result<usize> {
        let len = payload.len();
        match self.transport_tx.send(IoPkt { addr: to, payload }).await {
            Ok(_) => Ok(len),
            Err(e) => {
                bail!("failed to write IoPkt {:?}", e)
            }
        }
    }
    pub async fn recv_from(&self, data: &mut BytesMut) -> anyhow::Result<(usize, SocketAddrV4)> {
        match self.rx.recv().await {
            Ok(payload) => {
                if payload.payload.len() > data.remaining_mut() {
                    bail!(
                        "buffer not big enough expected {:?} had {:?} discarding packet",
                        payload.payload.len(),
                        data.remaining_mut()
                    )
                } else {
                    let len = payload.payload.len();
                    data.put(payload.payload);
                    Ok((len, payload.addr))
                }
            }
            Err(e) => {
                bail!("unable to received packet on this channel bailling {:?}", e);
            }
        }
    }
}
#[derive(Clone)]
pub struct WebRtcTransport {
    socket: UdpSocket,
    stun: IoPktChannel,
    dtls: IoPktChannel,
    rx: smol::channel::Receiver<IoPkt>,
    _tx_closer: smol::channel::Sender<()>,
    rx_closer: smol::channel::Receiver<()>,
}

impl Drop for WebRtcTransport {
    fn drop(&mut self) {
        let _ = self.stun.tx.close();
        let _ = self.stun.rx.close();
        let _ = self.dtls.tx.close();
        let _ = self.dtls.rx.close();
        let _ = self._tx_closer.close();
        let _ = self.rx_closer.close();
        self.rx.close();
    }
}

impl WebRtcTransport {
    pub fn new(socket: UdpSocket) -> Self {
        let (tx, rx) = smol::channel::bounded::<IoPkt>(10);
        let (_tx_closer, rx_closer) = smol::channel::bounded::<()>(1);
        Self {
            socket,
            stun: IoPktChannel::new(tx.clone()),
            dtls: IoPktChannel::new(tx),
            rx,
            rx_closer,
            _tx_closer,
        }
    }

    pub fn get_stun_channel(&self) -> anyhow::Result<IoPktChannel> {
        Ok(self.stun.clone())
    }
    pub fn get_dtls_channel(&self) -> anyhow::Result<IoPktChannel> {
        Ok(self.dtls.clone())
    }
    pub async fn read_loop(&self) {
        loop {
            let mut buf = [0; 1500];
            let r = futures_lite::future::or(self.socket.recv_from(&mut buf), async {
                let _ = self.rx_closer.recv().await;
                Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
            })
            .await;
            if r.is_err() {
                break;
            }
            let (len, addr) = r.unwrap();
            // TODO(npm) this is bad should be changed
            let buf = Bytes::copy_from_slice(&buf[..len]);
            log::debug!(
                "Packet recived b0 {:02X?} len {:?} from {:?}",
                buf[0],
                buf.len(),
                addr,
            );
            let addr = match addr {
                SocketAddr::V4(addr) => addr,
                _ => {
                    continue;
                }
            };
            if buf.len() >= 20 && buf[0] < 2 {
                if self
                    .stun
                    .send_pkt(IoPkt { addr, payload: buf })
                    .await
                    .is_err()
                {
                    break;
                }
            } else if self
                .dtls
                .send_pkt(IoPkt { addr, payload: buf })
                .await
                .is_err()
            {
                break;
            }
        }
        log::info!("bye read loop");
    }
    pub async fn write_loop(&self) {
        loop {
            let pkt = self.rx.recv().await;
            if pkt.is_err() {
                break;
            }
            let pkt = pkt.unwrap();
            match self.socket.send_to(pkt.payload.chunk(), pkt.addr).await {
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::OutOfMemory => {}
                Err(e) => {
                    log::error!("unexpected error {:?}", e);
                }
            }
        }
        log::info!("bye write loop");
    }
}
