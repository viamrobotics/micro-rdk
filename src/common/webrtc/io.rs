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
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_lite::{AsyncRead, AsyncWrite, Future, FutureExt};
use smol::net::UdpSocket;

#[derive(Debug)]
pub struct IoPkt {
    addr: SocketAddrV4,
    payload: Bytes,
}
#[derive(Clone)]
pub struct IoPktChannel {
    rx: smol::channel::Receiver<IoPkt>,
    transport_tx: smol::channel::Sender<IoPkt>,
    tx: smol::channel::Sender<IoPkt>,
    ip: Option<SocketAddrV4>,
    waker: Option<Waker>,
}

impl AsyncRead for IoPktChannel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        mut buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut recv = self.rx.recv();
        let fut = Box::pin(&mut recv).poll(cx);
        match fut {
            Poll::Ready(ret) => match ret {
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
            },
            Poll::Pending => {
                //TODO (npm) store revc future so that we poll it again in a not busy way
                // might be useful to use pin project
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

impl AsyncWrite for IoPktChannel {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let pkt = Bytes::copy_from_slice(buf);
        if let Some(ip) = self.ip.as_ref() {
            let ret = Pin::new(&mut self.transport_tx.send(IoPkt {
                addr: *ip,
                payload: pkt,
            }))
            .poll(cx);
            match ret {
                Poll::Ready(ret) => match ret {
                    Ok(_) => {
                        return Poll::Ready(Ok(buf.len()));
                    }
                    Err(e) => {
                        return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e)));
                    }
                },
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
        Poll::Ready(Ok(buf.len()))
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
        let (txo, rx) = smol::channel::bounded::<IoPkt>(3);
        Self {
            rx,
            transport_tx: tx,
            tx: txo,
            ip: None,
            waker: None,
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
}

impl WebRtcTransport {
    pub fn new(socket: UdpSocket) -> Self {
        let (tx, rx) = smol::channel::bounded::<IoPkt>(10);
        Self {
            socket,
            stun: IoPktChannel::new(tx.clone()),
            dtls: IoPktChannel::new(tx),
            rx,
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
            let (len, addr) = self.socket.recv_from(&mut buf).await.unwrap();
            // TODO(npm) this is bad should be changed
            let buf = Bytes::copy_from_slice(&buf[..len]);
            log::debug!(
                "Packet recived b0 {:02X?} len {:?} from {:?}",
                buf[0],
                buf.len(),
                addr
            );
            let addr = match addr {
                SocketAddr::V4(addr) => addr,
                _ => {
                    continue;
                }
            };
            if buf.len() >= 20 && buf[0] < 2 {
                self.stun
                    .send_pkt(IoPkt { addr, payload: buf })
                    .await
                    .unwrap();
            } else {
                self.dtls
                    .send_pkt(IoPkt { addr, payload: buf })
                    .await
                    .unwrap();
            }
        }
    }
    pub async fn write_loop(&self) {
        loop {
            let pkt = self.rx.recv().await.unwrap();
            match self.socket.send_to(pkt.payload.chunk(), pkt.addr).await {
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::OutOfMemory => {}
                Err(e) => {
                    log::error!("unexpected error {:?}", e);
                }
            }
        }
    }
}
