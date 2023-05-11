#![allow(dead_code)]
use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
    time::{Duration, Instant},
};

use async_channel::Sender;
use bytes::Bytes;

use futures_lite::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, Future};
use sctp_proto::{
    Association, AssociationHandle, Chunks, ClientConfig, DatagramEvent, Endpoint, EndpointConfig,
    Event, Payload, ServerConfig, StreamEvent, StreamId, Transmit,
};

use smol_timeout::TimeoutExt;

use super::exec::WebRtcExecutor;

//#[derive(Clone)]
struct SctpStream {
    data: VecDeque<Chunks>,
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
}

impl Channel {
    pub async fn write(&self, buf: &[u8]) {
        let bytes = Bytes::copy_from_slice(buf);

        self.tx_event
            .send(SctpEvent::OutgoingStreamData((self.tx_stream_id, bytes)))
            .await
            .unwrap();
    }
}

impl AsyncRead for Channel {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut rx_stream = self.rx_channel.lock().unwrap();
        if !rx_stream.data.is_empty() {
            let chunk = rx_stream.data.pop_front().unwrap();
            // TODO(RSDK-3062) : we assume that buf.len > chunk.len() this is wrong, we should do a
            // partial read an update remaining data accordingly
            let r = chunk.read(buf).unwrap();
            Poll::Ready(Ok(r))
        } else {
            let _ = rx_stream.waker.insert(cx.waker().clone());
            Poll::Pending
        }
    }
}

enum SctpEvent {
    IncomingData((SocketAddr, Bytes)),
    OutgoingData,
    Timeout(Instant),
    OutgoingStreamData((StreamId, Bytes)),
}

pub struct SctpProto<E, S> {
    endpoint: Endpoint,
    transport: S,
    association: Option<Association>,
    hnd: AssociationHandle,
    state: SctpState,
    sctp_event_rx: async_channel::Receiver<SctpEvent>,
    sctp_event_tx: async_channel::Sender<SctpEvent>,
    executor: E,
    channels: HashMap<ChannelId, Channel>,
    channels_rx: Sender<Channel>,
}

unsafe impl<S, E> Send for SctpProto<E, S> {}

impl<E, S> SctpProto<E, S>
where
    E: WebRtcExecutor<Pin<Box<dyn Future<Output = ()> + Send>>>,
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(transport: S, executor: E, channel_send: Sender<Channel>) -> Self {
        let endpoint_cfg = EndpointConfig::new();
        let endpoint = Endpoint::new(Arc::new(endpoint_cfg), None);

        let (sctp_event_tx, sctp_event_rx) = async_channel::unbounded();

        Self {
            endpoint,
            transport,
            association: None,
            hnd: AssociationHandle(0),
            state: SctpState::UnInit,
            sctp_event_rx,
            sctp_event_tx,
            executor,
            channels: HashMap::new(),
            channels_rx: channel_send,
        }
    }
    async fn write_to_transport(&mut self, transmit: Transmit) -> anyhow::Result<usize> {
        let written = match transmit.payload {
            Payload::RawEncode(data) => {
                let mut ret = 0;
                for payload in data {
                    ret += self.transport.write(&payload).await?;
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
    async fn handle_outgoing_data(&mut self) -> anyhow::Result<usize> {
        let mut written = 0;
        if let Some(pkt) = self.endpoint.poll_transmit() {
            written += self.write_to_transport(pkt).await?;
        }
        if let Some(assoc) = self.association.as_mut() {
            if let Some(pkt) = assoc.poll_transmit(Instant::now()) {
                written += self.write_to_transport(pkt).await?;
            }
        }
        Ok(written)
    }
    pub async fn listen(&mut self) -> anyhow::Result<()> {
        self.state = SctpState::AwaitAssociation;
        let server_config = Some(Arc::new(ServerConfig::new()));

        self.endpoint.set_server_config(server_config);

        Ok(())
    }
    pub async fn connect(&mut self, addr: SocketAddr) -> anyhow::Result<()> {
        let client_config = ClientConfig::new();

        let (hnd, assoc) = self.endpoint.connect(client_config, addr).unwrap();
        let _ = self.association.insert(assoc);
        self.hnd = hnd;
        if let Err(e) = self.sctp_event_tx.send(SctpEvent::OutgoingData).await {
            log::error!("When initiating an association event after an endpoint event couldn't submit event {:?}",e);
        }
        Ok(())
    }
    pub async fn run(&mut self) {
        let tx = self.sctp_event_tx.clone();
        //let mut transport = self.transport.clone();

        loop {
            let mut buf = [0; 1024];
            if let Some(len) = self
                .transport
                .read(&mut buf)
                .timeout(Duration::from_millis(15))
                .await
            {
                let len = len.unwrap();
                let buf = Bytes::copy_from_slice(&buf[..len]);
                // SCTP requires an ip field we set it to localhost 5000
                let from = "127.0.0.1:5000".parse().unwrap();
                tx.send(crate::common::webrtc::sctp::SctpEvent::IncomingData((
                    from, buf,
                )))
                .await
                .unwrap();
            }

            let event = if let Some(e) = self
                .sctp_event_rx
                .recv()
                .timeout(Duration::from_millis(19))
                .await
            {
                match e {
                    Ok(e) => e,
                    Err(e) => {
                        log::error!("Error waiting for event in an SCTP context {:?}", e);
                        continue;
                    }
                }
            } else {
                continue;
            };
            match event {
                SctpEvent::IncomingData((from, data)) => {
                    if let Some(ret) = self.endpoint.handle(Instant::now(), from, None, None, data)
                    {
                        if let Err(e) = self.process_datagram_event(ret.0, ret.1) {
                            log::error!("error while processing datagram event {:?}", e);
                        };
                    }
                }
                SctpEvent::OutgoingData => {
                    self.handle_outgoing_data().await.unwrap();
                }
                SctpEvent::Timeout(time) => {
                    if let Some(assoc) = self.association.as_mut() {
                        assoc.handle_timeout(time);
                    }
                }
                SctpEvent::OutgoingStreamData((id, buf)) => {
                    if let Some(assoc) = self.association.as_mut() {
                        if let Ok(mut stream) = assoc.stream(id) {
                            stream.write(&buf).unwrap();
                        } else {
                            log::error!("couldn't get stream .....");
                        }
                    }
                }
            };

            if let Some(assoc) = self.association.as_mut() {
                while let Some(ev) = assoc.poll() {
                    match ev {
                        Event::AssociationLost { reason } => {
                            log::error!("Association lost why? {:02x?}", reason);
                        }
                        Event::Connected => {
                            match assoc
                                .open_stream(0, sctp_proto::PayloadProtocolIdentifier::Binary)
                            {
                                Err(e) => {
                                    log::error!(" cannot open stream {:?}", e);
                                }
                                Ok(s) => {
                                    let c = Channel {
                                        tx_event: self.sctp_event_tx.clone(),
                                        tx_stream_id: s.stream_identifier(),
                                        rx_channel: Arc::new(Mutex::new(SctpStream {
                                            data: VecDeque::new(),
                                            waker: None,
                                        })),
                                    };
                                    self.channels.insert(ChannelId(0), c.clone());
                                    self.channels_rx.send(c).await.unwrap();
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
                                    let mut stream = channel.rx_channel.lock().unwrap();
                                    if let Ok(mut real_stream) = assoc.stream(id) {
                                        let data = real_stream.read().unwrap().unwrap();
                                        stream.data.push_back(data)
                                    }
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

                if let Some(endpoint) = assoc.poll_endpoint_event() {
                    if let Some(assoc_ev) = self.endpoint.handle_event(self.hnd, endpoint) {
                        assoc.handle_event(assoc_ev);
                        if let Err(e) = self.sctp_event_tx.send(SctpEvent::OutgoingData).await {
                            log::error!("When processing an association event after an endpoint event couldn't submit event {:?}",e);
                        }
                    }
                }
            }
            match self.handle_outgoing_data().await {
                Err(e) => {
                    log::error!("Error while sending data {:?}", e);
                }
                Ok(_size) => {
                    //log::info!("wrote {:?}", size);
                }
            }
            if let Some(assoc) = self.association.as_mut() {
                if let Some(timeout) = assoc.poll_timeout() {
                    let tx = self.sctp_event_tx.clone();
                    self.executor.execute(Box::pin(async move {
                        smol::Timer::at(timeout).await;
                        if let Err(e) = tx.send(SctpEvent::Timeout(Instant::now())).await {
                            log::error!("failed to send an SctpEvent {:?}", e);
                        }
                    }));
                }
            }
        }
    }

    fn process_datagram_event(
        &mut self,
        hnd: AssociationHandle,
        ev: DatagramEvent,
    ) -> anyhow::Result<()> {
        match ev {
            DatagramEvent::NewAssociation(assoc) => {
                let _ = self.association.insert(assoc);
                self.hnd = hnd;
                Ok(())
            }
            DatagramEvent::AssociationEvent(ev) => {
                if hnd != self.hnd {
                    log::error!("handles are different unexpected")
                }
                if let Some(assoc) = self.association.as_mut() {
                    assoc.handle_event(ev);
                }
                Ok(())
            }
        }
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

    use crate::common::webrtc::sctp::SctpProto;
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
        //UdpSocket::bind("127.0.0.1:63332").await.unwrap();

        log::error!("hellow");
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
        let mut srv = SctpProto::new(socket, exec.clone(), c_tx);

        let conn = srv.listen().await;

        assert!(conn.is_ok());

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
            channel.write(&buf[..read]).await;
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
        let mut client = SctpProto::new(socket, exec.clone(), c_tx);

        let ret = client.connect("127.0.0.1:63332".parse().unwrap()).await;

        exec.spawn(async move {
            client.run().await;
        })
        .detach();

        assert!(ret.is_ok());

        let channel = c_rx.recv().await;

        assert!(channel.is_ok());
        let mut channel = channel.unwrap();

        channel.write(b"hello").await;

        {
            let mut buf = [0; 8192];
            let read = channel.read(&mut buf).await;

            assert!(read.is_ok());

            let read = read.unwrap();

            assert_eq!(b"hello", &buf[..read]);
        }

        channel.write(b"hello world").await;

        {
            let mut buf = [0; 8192];
            let read = channel.read(&mut buf).await;

            assert!(read.is_ok());

            let read = read.unwrap();

            assert_eq!(b"hello world", &buf[..read]);
        }

        let random_bytes: Vec<u8> = (0..256).map(|_| rand::random::<u8>()).collect();

        channel.write(&random_bytes).await;
        {
            let mut buf = [0; 8192];
            let read = channel.read(&mut buf).await;

            assert!(read.is_ok());

            let read = read.unwrap();
            assert_eq!(&random_bytes, &buf[..read]);
        }
    }
}
