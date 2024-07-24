use super::{
    errors::ServerError,
    mdns::Mdns,
    network::Network,
    utils::{NoHttp2, WebRtcNoOp},
};
#[cfg(feature = "esp32")]
use crate::esp32::exec::Esp32Executor;
#[cfg(feature = "native")]
use crate::native::exec::NativeExecutor;
use crate::{
    common::{
        app_client::{
            AppClient, AppClientBuilder, AppClientConfig, AppClientError, AppSignaling,
            PeriodicAppClientTask,
        },
        grpc::{GrpcBody, GrpcServer},
        grpc_client::GrpcClient,
        robot::LocalRobot,
        webrtc::{
            api::{WebRtcApi, WebRtcError, WebRtcSdp},
            certificate::Certificate,
            dtls::{DtlsBuilder, DtlsConnector},
            exec::WebRtcExecutor,
            grpc::{WebRtcGrpcBody, WebRtcGrpcServer},
        },
    },
    proto::{self, app::v1::ConfigResponse},
};

use async_io::Timer;
use async_lock::{
    RwLock as AsyncRwLock, RwLockUpgradableReadGuard as AsyncRwLockUpgradableReadGuard,
};
use futures_lite::prelude::*;
use futures_lite::{future::Boxed, ready};
use hyper::{rt, server::conn::http2};

use async_executor::Task;
use std::{
    fmt::Debug,
    marker::PhantomData,
    net::Ipv4Addr,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};

#[cfg(feature = "native")]
type Executor = NativeExecutor;
#[cfg(feature = "esp32")]
type Executor = Esp32Executor;

pub trait TlsClientConnector {
    type Stream: rt::Read + rt::Write + Unpin + 'static;

    fn connect(&mut self) -> impl std::future::Future<Output = Result<Self::Stream, ServerError>>;
}

pub struct RobotCloudConfig {
    local_fqdn: String,
    name: String,
    fqdn: String,
}

impl RobotCloudConfig {
    pub fn new(local_fqdn: String, name: String, fqdn: String) -> Self {
        Self {
            local_fqdn,
            name,
            fqdn,
        }
    }
}

impl From<proto::app::v1::CloudConfig> for RobotCloudConfig {
    fn from(c: proto::app::v1::CloudConfig) -> Self {
        Self {
            local_fqdn: c.local_fqdn.clone(),
            name: c.local_fqdn.split('.').next().unwrap_or("").to_owned(),
            fqdn: c.fqdn.clone(),
        }
    }
}

impl From<&proto::app::v1::CloudConfig> for RobotCloudConfig {
    fn from(c: &proto::app::v1::CloudConfig) -> Self {
        Self {
            local_fqdn: c.local_fqdn.clone(),
            name: c.local_fqdn.split('.').next().unwrap_or("").to_owned(),
            fqdn: c.fqdn.clone(),
        }
    }
}

pub struct ViamServerBuilder<M, C, T, NetworkType, CC = WebRtcNoOp, D = WebRtcNoOp, L = NoHttp2> {
    mdns: M,
    webrtc: Option<Box<WebRtcConfiguration<D, CC>>>,
    port: u16, // gRPC/HTTP2 port
    http2_listener: L,
    _marker: PhantomData<T>,
    exec: Executor,
    app_connector: C,
    app_config: AppClientConfig,
    max_connections: usize,
    app_client_tasks: Vec<Box<dyn PeriodicAppClientTask>>,
    network: NetworkType,
    app_client: Option<AppClient>,
}

impl<M, C, T, NetworkType> ViamServerBuilder<M, C, T, NetworkType>
where
    M: Mdns,
    C: TlsClientConnector,
    T: rt::Read + rt::Write + Unpin + 'static,
    NetworkType: Network,
{
    pub fn new(
        mdns: M,
        exec: Executor,
        app_connector: C,
        app_config: AppClientConfig,
        max_connections: usize,
        network: NetworkType,
    ) -> Self {
        Self {
            mdns,
            http2_listener: NoHttp2 {},
            port: 0,
            webrtc: None,
            _marker: PhantomData,
            exec,
            app_connector,
            app_config,
            max_connections,
            app_client_tasks: vec![],
            network,
            app_client: None,
        }
    }
}

impl<M, C, T, NetworkType, CC, D, L> ViamServerBuilder<M, C, T, NetworkType, CC, D, L>
where
    M: Mdns,
    C: TlsClientConnector,
    T: rt::Read + rt::Write + Unpin + 'static,
    CC: Certificate + 'static,
    D: DtlsBuilder,
    D::Output: 'static,
    L: AsyncableTcpListener<T>,
    L::Output: Http2Connector<Stream = T>,
    NetworkType: Network,
{
    pub fn with_http2<L2, T2>(
        self,
        http2_listener: L2,
        port: u16,
    ) -> ViamServerBuilder<M, C, T2, NetworkType, CC, D, L2> {
        ViamServerBuilder {
            mdns: self.mdns,
            port,
            _marker: PhantomData,
            http2_listener,
            exec: self.exec,
            webrtc: self.webrtc,
            app_connector: self.app_connector,
            app_config: self.app_config,
            max_connections: self.max_connections,
            app_client_tasks: self.app_client_tasks,
            network: self.network,
            app_client: self.app_client,
        }
    }

    pub fn with_webrtc<D2, CC2>(
        self,
        webrtc: Box<WebRtcConfiguration<D2, CC2>>,
    ) -> ViamServerBuilder<M, C, T, NetworkType, CC2, D2, L> {
        ViamServerBuilder {
            mdns: self.mdns,
            webrtc: Some(webrtc),
            port: self.port,
            http2_listener: self.http2_listener,
            _marker: self._marker,
            exec: self.exec,
            app_connector: self.app_connector,
            app_config: self.app_config,
            max_connections: self.max_connections,
            app_client_tasks: self.app_client_tasks,
            network: self.network,
            app_client: self.app_client,
        }
    }

    pub fn with_app_client(
        self,
        app_client: AppClient,
    ) -> ViamServerBuilder<M, C, T, NetworkType, CC, D, L> {
        ViamServerBuilder {
            mdns: self.mdns,
            webrtc: self.webrtc,
            port: self.port,
            http2_listener: self.http2_listener,
            _marker: self._marker,
            exec: self.exec,
            app_connector: self.app_connector,
            app_config: self.app_config,
            max_connections: self.max_connections,
            app_client_tasks: self.app_client_tasks,
            network: self.network,
            app_client: Some(app_client),
        }
    }

    pub fn with_periodic_app_client_task(
        mut self,
        task: Box<dyn PeriodicAppClientTask>,
    ) -> ViamServerBuilder<M, C, T, NetworkType, CC, D, L> {
        ViamServerBuilder {
            mdns: self.mdns,
            webrtc: self.webrtc,
            port: self.port,
            http2_listener: self.http2_listener,
            _marker: self._marker,
            exec: self.exec,
            app_connector: self.app_connector,
            app_config: self.app_config,
            max_connections: self.max_connections,
            app_client_tasks: {
                self.app_client_tasks.push(task);
                self.app_client_tasks
            },
            network: self.network,
            app_client: self.app_client,
        }
    }

    pub fn build(
        mut self,
        config: &ConfigResponse,
    ) -> Result<ViamServer<C, T, CC, D, L, NetworkType>, ServerError> {
        let cfg: RobotCloudConfig = config
            .config
            .as_ref()
            .unwrap()
            .cloud
            .as_ref()
            .unwrap()
            .into();

        self.mdns
            .set_hostname(&cfg.name)
            .map_err(|e| ServerError::Other(e.into()))?;
        self.mdns
            .add_service(
                &cfg.local_fqdn.replace('.', "-"),
                "_rpc",
                "_tcp",
                self.port,
                &[("grpc", "")],
            )
            .map_err(|e| ServerError::Other(e.into()))?;
        self.mdns
            .add_service(
                &cfg.fqdn.replace('.', "-"),
                "_rpc",
                "_tcp",
                self.port,
                &[("grpc", "")],
            )
            .map_err(|e| ServerError::Other(e.into()))?;

        let cloned_exec = self.exec.clone();
        let http2_listener = HttpListener::new(self.http2_listener);

        let srv = ViamServer::new(
            http2_listener,
            self.webrtc,
            cloned_exec,
            self.app_connector,
            self.app_config,
            self.max_connections,
            self.app_client_tasks,
            self.network,
            self.app_client,
        );

        Ok(srv)
    }
}

pub trait Http2Connector: std::fmt::Debug {
    type Stream;
    fn accept(&mut self) -> impl std::future::Future<Output = std::io::Result<Self::Stream>>;
}

#[derive(Debug)]
pub struct HttpListener<L, T> {
    listener: L,
    marker: PhantomData<T>,
}
pin_project_lite::pin_project! {
    pub struct OwnedListener<T> {
        #[pin]
        pub inner: Boxed<std::io::Result<T>>,
    }
}

impl<T> Future for OwnedListener<T> {
    type Output = std::io::Result<T>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let r = ready!(this.inner.poll(cx));
        Poll::Ready(r)
    }
}

pub trait AsyncableTcpListener<T> {
    type Output: Debug + Http2Connector<Stream = T>;
    fn as_async_listener(&self) -> OwnedListener<Self::Output>;
}

impl<L, T> HttpListener<L, T>
where
    L: AsyncableTcpListener<T>,
{
    pub fn new(asyncable: L) -> Self {
        HttpListener {
            listener: asyncable,
            marker: PhantomData,
        }
    }
    fn next_conn(&self) -> OwnedListener<L::Output> {
        self.listener.as_async_listener()
    }
}

pub struct ViamServer<C, T, CC, D, L, NetworkType> {
    http_listener: HttpListener<L, T>,
    webrtc_config: Option<Box<WebRtcConfiguration<D, CC>>>,
    exec: Executor,
    app_connector: C,
    app_config: AppClientConfig,
    app_client: Rc<AsyncRwLock<Option<AppClient>>>,
    incoming_connection_manager: IncomingConnectionManager,
    app_client_tasks: Vec<Box<dyn PeriodicAppClientTask>>,
    network: NetworkType,
}
impl<C, T, CC, D, L, NetworkType> ViamServer<C, T, CC, D, L, NetworkType>
where
    C: TlsClientConnector,
    T: rt::Read + rt::Write + Unpin + 'static,
    CC: Certificate + 'static,
    D: DtlsBuilder,
    D::Output: 'static,
    L: AsyncableTcpListener<T>,
    L::Output: Http2Connector<Stream = T>,
    NetworkType: Network,
{
    #[allow(clippy::too_many_arguments)]
    fn new(
        http_listener: HttpListener<L, T>,
        webrtc_config: Option<Box<WebRtcConfiguration<D, CC>>>,
        exec: Executor,
        app_connector: C,
        app_config: AppClientConfig,
        max_concurent_connections: usize,
        app_client_tasks: Vec<Box<dyn PeriodicAppClientTask>>,
        network: NetworkType,
        app_client: Option<AppClient>,
    ) -> Self {
        Self {
            http_listener,
            webrtc_config,
            exec,
            app_connector,
            app_config,
            app_client: Rc::new(AsyncRwLock::new(app_client)),
            incoming_connection_manager: IncomingConnectionManager::new(max_concurent_connections),
            app_client_tasks,
            network,
        }
    }
    pub async fn serve(&mut self, robot: Arc<Mutex<LocalRobot>>) {
        let cloned_robot = robot.clone();

        // Let the robot register any periodic app client tasks it may
        // have based on its configuration.
        self.app_client_tasks
            .append(&mut robot.lock().unwrap().get_periodic_app_client_tasks());

        // Convert each `PeriodicAppClientTask` implementer into an async task spawned on the
        // executor, and collect them all into `_app_client_tasks` so we don't lose track of them.
        let _app_client_tasks: Vec<_> = self
            .app_client_tasks
            .drain(..)
            .map(|mut task| {
                // Each task gets a handle to the `RwLock` wrapped `Option` that (might) contain an
                // `AppClient`.
                let app_client = Rc::clone(&self.app_client);
                self.exec.spawn(async move {
                    // Start the wait duration per task implementer. If a task execution returns a
                    // new duration, update `duration` to the new value so that it will be used
                    // until next updated.
                    let mut duration = task.get_default_period();
                    loop {
                        // Wait for the period to expire, then inspect the state of the `AppClient`
                        // under the read lock. If there is currently an `AppClient` in play, let
                        // the task use it to conduct it's business. If the task returns a new wait
                        // duration, update `duration`. Otherwise, just sleep again and hope that an
                        // `AppClient` will be available on the next wakeup.
                        let _ = async_io::Timer::after(duration).await;
                        let urguard = app_client.upgradable_read().await;
                        for app_client in urguard.as_ref().iter() {
                            match task.invoke(app_client).await {
                                Ok(None) => continue,
                                Ok(Some(next_duration)) => {
                                    duration = next_duration;
                                }
                                Err(e) => {
                                    log::error!(
                                        "Periodic task {} failed with error {:?} - dropping app client",
                                        task.name(),
                                        e
                                    );
                                    let _ = AsyncRwLockUpgradableReadGuard::upgrade(urguard).await.take();
                                    break;
                                }
                            }
                        }
                    }
                })
            })
            .collect();

        loop {
            let _ = async_io::Timer::after(std::time::Duration::from_millis(300)).await;
            if !self.network.is_connected().unwrap_or(false) {
                // the IP may change on reconnection (such as in the case where we are disconnected
                // from a Wi-Fi base station), so we want to prompt the creation of a new
                // app client
                let _ = self.app_client.write().await.take();
                continue;
            }
            {
                let urguard = self.app_client.upgradable_read().await;
                if urguard.is_none() {
                    let conn = match self.app_connector.connect().await {
                        Ok(conn) => conn,
                        Err(err) => {
                            log::error!("failure to connect: {:?}", err);
                            continue;
                        }
                    };
                    let cloned_exec = self.exec.clone();
                    let grpc_client = Box::new(
                        GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443")
                            .await
                            .unwrap(),
                    );
                    let app_client = AppClientBuilder::new(grpc_client, self.app_config.clone())
                        .build()
                        .await
                        .unwrap();
                    let _ = AsyncRwLockUpgradableReadGuard::upgrade(urguard)
                        .await
                        .insert(app_client);
                }
            }
            let sig = if let Some(webrtc_config) = self.webrtc_config.as_ref() {
                let ip = self.network.get_ip();
                let rguard = self.app_client.read().await;
                let signaling = rguard.as_ref().unwrap().initiate_signaling();
                futures_util::future::Either::Left(WebRTCSignalingAnswerer {
                    webrtc_config: Some(webrtc_config),
                    future: signaling,
                    ip,
                })
            } else {
                futures_util::future::Either::Right(WebRTCSignalingAnswerer::<
                    '_,
                    CC,
                    D,
                    futures_lite::future::Pending<Result<AppSignaling, AppClientError>>,
                >::default())
            };
            let listener = self.http_listener.next_conn();
            log::info!("waiting for connection");

            let connection = futures_lite::future::or(
                async move {
                    let p = listener.await;
                    p.map(IncomingConnection::Http2Connection)
                        .map_err(|e| ServerError::Other(e.into()))
                },
                async {
                    let mut api = sig.await?;

                    let prio = self.incoming_connection_manager.get_lowest_prio();

                    let sdp = api
                        .answer(prio)
                        .await
                        .map_err(ServerError::ServerWebRTCError)?;

                    Ok(IncomingConnection::WebRtcConnection(WebRTCConnection {
                        webrtc_api: api,
                        sdp: sdp.0,
                        server: None,
                        robot: cloned_robot.clone(),
                        prio: sdp.1,
                    }))
                },
            );
            let connection = connection
                .or(async {
                    Timer::after(Duration::from_secs(600)).await;
                    Err(ServerError::ServerConnectionTimeout)
                })
                .await;

            let connection = match connection {
                Ok(c) => c,
                Err(ServerError::ServerWebRTCError(_))
                | Err(ServerError::ServerConnectionTimeout) => {
                    // all webrtc/timeout errors don't require a tls renegotiation
                    continue;
                }
                Err(_) => {
                    // http2 layer related errors (GOAWAY etc...) so we should renegotiate in this event
                    let _ = self.app_client.write().await.take();
                    continue;
                }
            };
            if let Err(e) = match connection {
                IncomingConnection::Http2Connection(mut c) => match c.accept().await {
                    Err(e) => Err(ServerError::Other(e.into())),
                    Ok(c) => {
                        let robot = robot.clone();
                        let exec = self.exec.clone();
                        let t = self
                            .exec
                            .spawn(async move { Self::serve_http2(c, exec, robot).await });
                        // Incoming direct HTTP2 connections take top priority.
                        self.incoming_connection_manager
                            .insert_new_conn(t, u32::MAX)
                            .await;
                        Ok(())
                    }
                },
                IncomingConnection::WebRtcConnection(mut c) => match c.open_data_channel().await {
                    Err(e) => Err(e),
                    Ok(_) => {
                        let prio = c.prio;
                        let t = self.exec.spawn(async move { c.run().await });
                        self.incoming_connection_manager
                            .insert_new_conn(t, prio)
                            .await;
                        Ok(())
                    }
                },
            } {
                log::error!("error while serving {}", e);
            }
        }
    }
    async fn serve_http2(
        connection: T,
        exec: Executor,
        robot: Arc<Mutex<LocalRobot>>,
    ) -> Result<(), ServerError> {
        let srv = GrpcServer::new(robot.clone(), GrpcBody::new());
        Box::new(
            http2::Builder::new(exec)
                .initial_connection_window_size(2048)
                .initial_stream_window_size(2048)
                .max_send_buf_size(4096)
                .max_concurrent_streams(2)
                .serve_connection(connection, srv),
        )
        .await
        .map_err(|e| ServerError::Other(e.into()))
    }
}
#[derive(Debug)]
pub enum IncomingConnection<L, U> {
    Http2Connection(L),
    WebRtcConnection(U),
}

#[derive(Clone)]
pub struct WebRtcConfiguration<D, CC> {
    pub dtls: D,
    pub cert: Rc<CC>,
    pub exec: Executor,
}

impl<D, CC> WebRtcConfiguration<D, CC>
where
    D: DtlsBuilder,
    CC: Certificate,
{
    pub fn new(cert: Rc<CC>, dtls: D, exec: Executor) -> Self {
        Self { dtls, cert, exec }
    }
}
struct WebRTCConnection<C, D, E> {
    webrtc_api: WebRtcApi<C, D, E>,
    sdp: Box<WebRtcSdp>,
    server: Option<WebRtcGrpcServer<GrpcServer<WebRtcGrpcBody>>>,
    robot: Arc<Mutex<LocalRobot>>,
    prio: u32,
}

impl<C, D, E> WebRTCConnection<C, D, E>
where
    C: Certificate,
    D: DtlsConnector,
    E: WebRtcExecutor<Pin<Box<dyn Future<Output = ()>>>> + Clone,
{
    async fn open_data_channel(&mut self) -> Result<(), ServerError> {
        self.webrtc_api
            .run_ice_until_connected(&self.sdp)
            .or(async {
                Timer::after(Duration::from_secs(10)).await;
                Err(WebRtcError::OperationTiemout)
            })
            .await
            .map_err(|e| match e {
                WebRtcError::OperationTiemout => ServerError::ServerConnectionTimeout,
                _ => ServerError::Other(e.into()),
            })?;

        let c = self
            .webrtc_api
            .open_data_channel()
            .or(async {
                Timer::after(Duration::from_secs(10)).await;
                Err(WebRtcError::OperationTiemout)
            })
            .await
            .map_err(|e| match e {
                WebRtcError::OperationTiemout => ServerError::ServerConnectionTimeout,
                _ => ServerError::Other(e.into()),
            })?;
        let srv = WebRtcGrpcServer::new(
            c,
            GrpcServer::new(self.robot.clone(), WebRtcGrpcBody::default()),
        );
        let _ = self.server.insert(srv);
        Ok(())
    }
    async fn run(&mut self) -> Result<(), ServerError> {
        if self.server.is_none() {
            return Err(ServerError::ServerConnectionNotConfigured);
        }
        let srv = self.server.as_mut().unwrap();
        loop {
            let req = srv
                .next_request()
                .or(async {
                    Timer::after(Duration::from_secs(30)).await;
                    Err(WebRtcError::OperationTiemout)
                })
                .await;

            if let Err(e) = req {
                return Err(ServerError::Other(Box::new(e)));
            }
        }
    }
}

pin_project_lite::pin_project! {
    struct WebRTCSignalingAnswerer<'a, C,D,F> {
        #[pin]
        future: F,
        webrtc_config: Option<&'a WebRtcConfiguration<D,C>>,
        ip: Ipv4Addr,
    }
}

impl<'a, C, D, F> WebRTCSignalingAnswerer<'a, C, D, F> {
    fn default(
    ) -> WebRTCSignalingAnswerer<'a, C, D, impl Future<Output = Result<AppSignaling, AppClientError>>>
    {
        WebRTCSignalingAnswerer {
            future: futures_lite::future::pending::<Result<AppSignaling, AppClientError>>(),
            webrtc_config: None,
            ip: Ipv4Addr::new(0, 0, 0, 0),
        }
    }
}

impl<'a, C, D, F> Future for WebRTCSignalingAnswerer<'a, C, D, F>
where
    F: Future<Output = Result<AppSignaling, AppClientError>>,
    C: Certificate,
    D: DtlsBuilder,
{
    type Output = Result<WebRtcApi<C, D::Output, Executor>, ServerError>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let r = ready!(this.future.poll(cx));
        let s = match r {
            Err(e) => return Poll::Ready(Err(ServerError::ServerAppClientError(e))),
            Ok(s) => s,
        };
        Poll::Ready(Ok(WebRtcApi::new(
            this.webrtc_config.as_ref().unwrap().exec.clone(),
            s.0,
            s.1,
            this.webrtc_config.as_ref().unwrap().cert.clone(),
            *this.ip,
            this.webrtc_config.as_ref().unwrap().dtls.make().unwrap(),
        )))
    }
}

#[derive(Default)]
struct IncomingConnectionTask {
    task: Option<Task<Result<(), ServerError>>>,
    prio: Option<u32>,
}

impl IncomingConnectionTask {
    fn replace(&mut self, task: Task<Result<(), ServerError>>, prio: u32) {
        let _ = self.task.replace(task);
        let _ = self.prio.replace(prio);
    }
    fn is_finished(&self) -> bool {
        if let Some(task) = self.task.as_ref() {
            return task.is_finished();
        }
        true
    }
    async fn cancel(&mut self) -> Option<ServerError> {
        if let Some(task) = self.task.take() {
            return task.cancel().await?.err();
        }
        None
    }
    fn get_prio(&self) -> u32 {
        if !self.is_finished() {
            return *self.prio.as_ref().unwrap_or(&0);
        }
        0
    }
}

struct IncomingConnectionManager {
    connections: Vec<IncomingConnectionTask>,
}

impl IncomingConnectionManager {
    fn new(size: usize) -> Self {
        let mut connections = Vec::with_capacity(size);
        connections.resize_with(size, Default::default);
        Self { connections }
    }
    // return the lowest priority of active webrtc tasks or 0
    fn get_lowest_prio(&self) -> u32 {
        self.connections
            .iter()
            .min_by(|a, b| a.get_prio().cmp(&b.get_prio()))
            .map_or(0, |c| c.get_prio())
    }
    // function will never fail and the lowest priority will always be replaced
    async fn insert_new_conn(&mut self, task: Task<Result<(), ServerError>>, prio: u32) {
        if let Some(slot) = self
            .connections
            .iter_mut()
            .min_by(|a, b| a.get_prio().cmp(&b.get_prio()))
        {
            if let Some(last_error) = slot.cancel().await {
                log::info!("last_error {:?}", last_error);
            }
            slot.replace(task, prio);
        }
    }
}
