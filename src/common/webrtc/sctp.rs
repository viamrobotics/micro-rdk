#![allow(dead_code)]
use std::{
    collections::HashMap,
    fmt::Debug,
    io,
    net::SocketAddr,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
    time::Instant,
};

use async_channel::Sender;
use bytes::Bytes;
use thiserror::Error;

use futures_lite::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use sctp_proto::{
    Association, AssociationHandle, ClientConfig, DatagramEvent, Endpoint, EndpointConfig, Event,
    Payload, ServerConfig, StreamEvent, StreamId, Transmit,
};

//#[derive(Clone)]
struct SctpStream {
    waker: Option<Waker>,
}

impl Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel").finish()
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct ChannelId(u16);
#[derive(Clone)]
pub struct Channel {
    tx_event: Sender<SctpEvent>,
    tx_stream_id: StreamId,
    rx_channel: Arc<Mutex<SctpStream>>,
    association: Arc<Mutex<Association>>,
    closed: Arc<Mutex<bool>>,
}

impl Channel {
    pub async fn write(&self, buf: &[u8]) -> std::io::Result<()> {
        if *self.closed.lock().unwrap() {
            return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        }
        let bytes = Bytes::copy_from_slice(buf);
        self.tx_event
            .send(SctpEvent::OutgoingStreamData((self.tx_stream_id, bytes)))
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

impl AsyncRead for Channel {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        if *self.closed.lock().unwrap() {
            return Poll::Ready(Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)));
        }
        let mut association = self.association.lock().unwrap();
        let mut stream = association
            .stream(self.tx_stream_id)
            .map_err(|_| std::io::ErrorKind::BrokenPipe)?;

        if let Some(chunk) = stream
            .read_sctp()
            .map_err(|_| std::io::ErrorKind::BrokenPipe)?
        {
            let r = chunk.read(buf).unwrap();
            return Poll::Ready(Ok(r));
        }
        let mut rx_stream = self.rx_channel.lock().unwrap();
        let _ = rx_stream.waker.insert(cx.waker().clone());
        Poll::Pending
    }
}

#[derive(Debug)]
enum SctpEvent {
    IncomingData((SocketAddr, Bytes)),
    OutgoingData,
    Timeout(Instant),
    OutgoingStreamData((StreamId, Bytes)),
    Disconnect,
}

#[derive(Error, Debug)]
pub enum SctpError {
    #[error("couldn't accept connection")]
    SctpErrorCannotAssociate,
    #[error("couldn't connect")]
    SctpErrorCouldntConnect,
    #[error(transparent)]
    SctpIoError(#[from] io::Error),
    #[error(transparent)]
    SctpErrorConnect(#[from] sctp_proto::ConnectError),
    #[error("Sctp event queue full")]
    SctpErrorEventQueueFull,
    #[error("Sctp connection closed")]
    SctpDisconnected,
}

pub struct SctpConnector<S> {
    endpoint: Endpoint,
    state: SctpState,
    transport: S,
    channels_rx: Sender<Channel>,
}

impl<S> SctpConnector<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    pub fn new(transport: S, channel_send: Sender<Channel>) -> Self {
        let endpoint_cfg = EndpointConfig::new();
        let endpoint = Endpoint::new(Arc::new(endpoint_cfg), None);
        Self {
            endpoint,
            state: SctpState::UnInit,
            channels_rx: channel_send,
            transport,
        }
    }
    pub async fn listen(mut self) -> Result<SctpProto<S>, SctpError> {
        self.state = SctpState::AwaitAssociation;
        let server_config = Some(Arc::new(ServerConfig::new()));

        self.endpoint.set_server_config(server_config);

        let mut buf = [0; 300];

        let len = self
            .transport
            .read(&mut buf)
            .await
            .map_err(SctpError::SctpIoError)?;

        if len == 0 {
            return Err(SctpError::SctpErrorCannotAssociate);
        }

        let buf = Bytes::copy_from_slice(&buf[..len]);
        let from = "127.0.0.1:5000".parse().unwrap();

        let (hnd, mut assoc) = if let Some((hnd, DatagramEvent::NewAssociation(assoc))) =
            self.endpoint.handle(Instant::now(), from, None, None, buf)
        {
            (hnd, assoc)
        } else {
            return Err(SctpError::SctpErrorCannotAssociate);
        };

        if let Some(pkt) = assoc.poll_transmit(Instant::now()) {
            let _ = match pkt.payload {
                Payload::RawEncode(data) => {
                    let mut ret = 0;
                    for payload in data {
                        ret += self.transport.write(&payload).await?;
                    }
                    ret
                }
                _ => {
                    return Err(SctpError::SctpErrorCannotAssociate);
                }
            };
        }

        let (sctp_event_tx, sctp_event_rx) = async_channel::unbounded();
        Ok(SctpProto {
            endpoint: self.endpoint,
            transport: self.transport,
            association: Arc::new(Mutex::new(assoc)),
            hnd,
            state: SctpState::AwaitAssociation,
            sctp_event_rx,
            sctp_event_tx,
            channels: HashMap::new(),
            channels_rx: self.channels_rx,
        })
    }

    pub async fn connect(mut self, addr: SocketAddr) -> Result<SctpProto<S>, SctpError> {
        let client_config = ClientConfig::new();

        let (hnd, mut association) = self
            .endpoint
            .connect(client_config, addr)
            .map_err(SctpError::SctpErrorConnect)?;

        if let Some(pkt) = association.poll_transmit(Instant::now()) {
            let _ = match pkt.payload {
                Payload::RawEncode(data) => {
                    let mut ret = 0;
                    for payload in data {
                        ret += self.transport.write(&payload).await?;
                    }
                    ret
                }
                _ => {
                    return Err(SctpError::SctpErrorCannotAssociate);
                }
            };
        }

        let (sctp_event_tx, sctp_event_rx) = async_channel::bounded(5);
        Ok(SctpProto {
            endpoint: self.endpoint,
            transport: self.transport,
            association: Arc::new(Mutex::new(association)),
            hnd,
            state: SctpState::AwaitAssociation,
            sctp_event_rx,
            sctp_event_tx,
            channels: HashMap::new(),
            channels_rx: self.channels_rx,
        })
    }
}

pub struct SctpHandle {
    sctp_event_tx: async_channel::Sender<SctpEvent>,
}

impl SctpHandle {
    pub fn close(&mut self) -> Result<(), SctpError> {
        self.sctp_event_tx
            .try_send(SctpEvent::Disconnect)
            .map_err(|_| SctpError::SctpDisconnected)
    }
}

pub struct SctpProto<S> {
    endpoint: Endpoint,
    transport: S,
    association: Arc<Mutex<Association>>,
    hnd: AssociationHandle,
    state: SctpState,
    sctp_event_rx: async_channel::Receiver<SctpEvent>,
    sctp_event_tx: async_channel::Sender<SctpEvent>,
    channels: HashMap<ChannelId, Channel>,
    channels_rx: Sender<Channel>,
}

impl<S> Drop for SctpProto<S> {
    fn drop(&mut self) {
        log::debug!("drop sctp");
        let _ = self.sctp_event_rx.close();
    }
}

unsafe impl<S> Send for SctpProto<S> {}

async fn write_to_transport<S: AsyncRead + AsyncWrite + Unpin + Send>(
    mut transport: S,
    transmit: Transmit,
) -> anyhow::Result<usize> {
    let written = match transmit.payload {
        Payload::RawEncode(data) => {
            let mut ret = 0;
            for payload in data {
                ret += transport.write(&payload).await?;
            }
            ret
        }
        Payload::PartialDecode(data) => {
            log::error!(
                "received a Partial decoded but don't know what to do with it {:?}",
                data
            );
            0
        }
    };
    Ok(written)
}

impl<S> SctpProto<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    fn close(&mut self) -> Result<(), SctpError> {
        self.sctp_event_tx
            .send_blocking(SctpEvent::Disconnect)
            .map_err(|_| SctpError::SctpErrorEventQueueFull)
    }

    fn process_association_events(&mut self) -> Result<(), SctpError> {
        let mut association = self.association.lock().unwrap();
        while let Some(ev) = association.poll() {
            match ev {
                Event::AssociationLost { reason } => {
                    log::error!("Association lost why? {:02x?}", reason);
                    let _ = association.close();
                    break;
                }
                Event::Connected => {
                    match association.open_stream(0, sctp_proto::PayloadProtocolIdentifier::Binary)
                    {
                        Err(e) => {
                            log::error!(" cannot open stream {:?}", e);
                        }
                        Ok(s) => {
                            let c = Channel {
                                tx_event: self.sctp_event_tx.clone(),
                                tx_stream_id: s.stream_identifier(),
                                rx_channel: Arc::new(Mutex::new(SctpStream { waker: None })),
                                closed: Arc::new(Mutex::new(false)),
                                association: self.association.clone(),
                            };
                            self.channels.insert(ChannelId(0), c.clone());
                            if let Err(e) = self.channels_rx.try_send(c) {
                                log::error!("Failed to send opened channel {:?}", e);
                            }
                        }
                    }
                }
                Event::DatagramReceived => {
                    log::debug!("we have received some data on this association");
                }
                Event::Stream(stream) => match stream {
                    StreamEvent::Opened => {
                        log::debug!("some stream was opened")
                    }
                    StreamEvent::Readable { id } => {
                        if let Some(channel) = self.channels.get(&ChannelId(id)) {
                            let stream = channel.rx_channel.lock().unwrap();

                            if let Some(waker) = stream.waker.as_ref() {
                                waker.clone().wake();
                            }
                        }
                    }
                    _ => {
                        log::debug!("skipping this stream event {:?}", stream)
                    }
                },
            }
        }
        Ok(())
    }

    pub fn get_handle(&self) -> SctpHandle {
        SctpHandle {
            sctp_event_tx: self.sctp_event_tx.clone(),
        }
    }

    async fn process_endpoint_events(&mut self) -> Result<(), SctpError> {
        {
            let mut association = self.association.lock().unwrap();
            if let Some(endpoint) = association.poll_endpoint_event() {
                if let Some(assoc_ev) = self.endpoint.handle_event(self.hnd, endpoint) {
                    association.handle_event(assoc_ev);
                    if let Err(e) = self.sctp_event_tx.try_send(SctpEvent::OutgoingData) {
                        log::error!("When processing an association event after an endpoint event couldn't submit event {:?}",e);
                    }
                }
            }
        }
        if let Some(pkt) = self.endpoint.poll_transmit() {
            let _ = write_to_transport(&mut self.transport, pkt).await;
        }
        Ok(())
    }

    async fn send_association_packets(&mut self) -> Result<(), SctpError> {
        let pkt = {
            self.association
                .lock()
                .unwrap()
                .poll_transmit(Instant::now())
        };
        if let Some(pkt) = pkt {
            let _ = write_to_transport(&mut self.transport, pkt).await;
        }

        Ok(())
    }
    pub async fn run(&mut self) {
        let mut sctp_timeout = None;
        loop {
            let mut buf = [0; 1500];
            let timeout = sctp_timeout
                .take()
                .map_or_else(smol::Timer::never, smol::Timer::at);
            let event = futures_lite::future::or(
                async {
                    match self.sctp_event_rx.recv().await {
                        Ok(e) => e,
                        Err(_) => SctpEvent::Disconnect,
                    }
                },
                futures_lite::future::or(
                    async {
                        let r = self.transport.read(&mut buf).await;
                        if r.is_err() {
                            return SctpEvent::Disconnect;
                        }
                        let len = r.unwrap();
                        if len == 0 {
                            return SctpEvent::Disconnect;
                        }

                        let buf = Bytes::copy_from_slice(&buf[..len]);
                        let from = "127.0.0.1:5000".parse().unwrap();

                        SctpEvent::IncomingData((from, buf))
                    },
                    async {
                        timeout.await;
                        log::debug!("TIMEOUT");
                        SctpEvent::Timeout(Instant::now())
                    },
                ),
            )
            .await;

            match event {
                SctpEvent::IncomingData((from, data)) => {
                    if let Some((hnd, ev)) =
                        self.endpoint.handle(Instant::now(), from, None, None, data)
                    {
                        match ev {
                            DatagramEvent::NewAssociation(_) => {}
                            DatagramEvent::AssociationEvent(ev) => {
                                if hnd != self.hnd {
                                    log::error!(
                                        "the association handle of the datagram is not the one active currently"
                                    );
                                } else {
                                    self.association.lock().unwrap().handle_event(ev);
                                }
                            }
                        };
                    }
                }
                SctpEvent::OutgoingData => {}
                SctpEvent::Timeout(time) => {
                    let mut association = self.association.lock().unwrap();
                    association.handle_timeout(time);
                }
                SctpEvent::OutgoingStreamData((id, buf)) => {
                    let mut association = self.association.lock().unwrap();
                    if let Ok(mut stream) = association.stream(id) {
                        let _ = stream.write(&buf);
                    } else {
                        log::error!("couldn't get stream .....");
                    }
                }
                SctpEvent::Disconnect => {
                    let mut association = self.association.lock().unwrap();
                    let _ = association.close();
                    break;
                }
            };

            self.process_association_events().unwrap();
            self.process_endpoint_events().await.unwrap();
            self.send_association_packets().await.unwrap();

            if let Some(timeout) = self.association.lock().unwrap().poll_timeout() {
                //log::error!("next timeout {:?}", timeout);
                let _ = sctp_timeout.insert(timeout);
            }
        }

        for channel in &self.channels {
            *channel.1.closed.lock().unwrap() = true;
            if let Some(waker) = &channel.1.rx_channel.lock().unwrap().waker {
                waker.wake_by_ref();
            }
        }
        let _ = self.sctp_event_tx.close();
        let _ = self.sctp_event_rx.close();
    }
}

enum SctpState {
    UnInit,
    AwaitAssociation,
    AssociationRequested,
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::Poll;
    use std::time::Duration;

    use crate::common::webrtc::sctp::SctpConnector;
    use crate::native::exec::NativeExecutor;
    use async_io::{Async, Timer};
    use futures_lite::future::block_on;
    use futures_lite::AsyncReadExt;
    use futures_lite::{ready, AsyncRead, AsyncWrite, Future};

    struct UdpStreamAdapter {
        inner: Arc<Async<std::net::UdpSocket>>,
        local: SocketAddr,
        peer: SocketAddr,
        readable: Option<async_io::ReadableOwned<std::net::UdpSocket>>,
        writable: Option<async_io::WritableOwned<std::net::UdpSocket>>,
    }

    impl UdpStreamAdapter {
        fn new(socket: std::net::UdpSocket, local: SocketAddr, peer: SocketAddr) -> Self {
            Self {
                inner: Arc::new(Async::new(socket).unwrap()),
                readable: None,
                writable: None,
                local,
                peer,
            }
        }
    }

    impl AsyncRead for UdpStreamAdapter {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            loop {
                match self.inner.get_ref().recv_from(buf) {
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    res => {
                        let _ = self.readable.take();
                        if let Ok(s) = &res {
                            if s.1 != self.peer {
                                continue;
                            }
                        }
                        let res = res.map(|s| s.0);
                        return Poll::Ready(res);
                    }
                }

                if self.readable.is_none() {
                    self.readable = Some(self.inner.clone().readable_owned());
                }
                if let Some(f) = &mut self.readable {
                    let res = ready!(Pin::new(f).poll(cx));
                    self.readable = None;
                    res?;
                }
            }
        }
    }

    impl AsyncWrite for UdpStreamAdapter {
        fn poll_flush(
            self: Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_close(
            self: Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            loop {
                match self.inner.get_ref().send_to(buf, self.peer) {
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    res => {
                        return Poll::Ready(res);
                    }
                }

                if self.writable.is_none() {
                    self.writable = Some(self.inner.clone().writable_owned());
                }

                if let Some(f) = &mut self.writable {
                    let res = ready!(Pin::new(f).poll(cx));
                    self.writable = None;
                    res?;
                }
            }
        }
    }

    #[test_log::test]
    fn test_sctp() {
        let local_ex = NativeExecutor::new();

        let cloned = local_ex.clone();
        let cloned2 = local_ex.clone();
        local_ex
            .spawn(async move { run_server_echo(cloned).await })
            .detach();

        block_on(local_ex.run(async move { run_client(cloned2).await }));
    }

    async fn run_server_echo(exec: NativeExecutor<'_>) {
        let socket = std::net::UdpSocket::bind("127.0.0.1:63332");

        assert!(socket.is_ok());

        let socket = socket.unwrap();

        let socket = UdpStreamAdapter::new(
            socket,
            "127.0.0.1:63332".parse().unwrap(),
            "127.0.0.1:63333".parse().unwrap(),
        );

        let (c_tx, c_rx) = async_channel::unbounded();
        let srv = SctpConnector::new(socket, c_tx);

        let conn = srv.listen().await;

        assert!(conn.is_ok());

        let mut srv = conn.unwrap();

        exec.spawn(async move {
            srv.run().await;
        })
        .detach();

        let channel = c_rx.recv().await;

        assert!(channel.is_ok());
        let mut channel = channel.unwrap();

        loop {
            let mut buf = [0; 8192];

            let read = channel.read(&mut buf).await;

            assert!(read.is_ok());

            let read = read.unwrap();
            assert!(channel.write(&buf[..read]).await.is_ok());
        }
    }

    async fn run_client(exec: NativeExecutor<'_>) {
        // let server spawn
        Timer::after(Duration::from_millis(100)).await;
        let socket = std::net::UdpSocket::bind("127.0.0.1:63333").unwrap();

        let socket = UdpStreamAdapter::new(
            socket,
            "127.0.0.1:63333".parse().unwrap(),
            "127.0.0.1:63332".parse().unwrap(),
        );

        let (c_tx, c_rx) = async_channel::unbounded();
        let client = SctpConnector::new(socket, c_tx);

        let ret = client.connect("127.0.0.1:63332".parse().unwrap()).await;

        assert!(ret.is_ok());

        let mut client = ret.unwrap();
        let mut hnd = client.get_handle();

        exec.spawn(async move {
            client.run().await;
        })
        .detach();

        let channel = c_rx.recv().await;

        assert!(channel.is_ok());
        let mut channel = channel.unwrap();

        assert!(channel.write(b"hello").await.is_ok());

        {
            let mut buf = [0; 8192];
            let read = channel.read(&mut buf).await;

            assert!(read.is_ok());

            let read = read.unwrap();

            assert_eq!(b"hello", &buf[..read]);
        }

        assert!(channel.write(b"hello world").await.is_ok());

        {
            let mut buf = [0; 8192];
            let read = channel.read(&mut buf).await;

            assert!(read.is_ok());

            let read = read.unwrap();

            assert_eq!(b"hello world", &buf[..read]);
        }

        let random_bytes: Vec<u8> = (0..4096).map(|_| rand::random::<u8>()).collect();

        assert!(channel.write(&random_bytes).await.is_ok());
        {
            let mut buf = [0; 8192];
            let read = channel.read(&mut buf).await;

            assert!(read.is_ok());

            let read = read.unwrap();
            assert_eq!(&random_bytes, &buf[..read]);
        }

        {
            let ret = hnd.close();
            assert!(ret.is_ok());
            let mut buf = [0; 8192];
            let read = channel.read(&mut buf).await;
            assert!(read.is_err());
        }
    }
}
