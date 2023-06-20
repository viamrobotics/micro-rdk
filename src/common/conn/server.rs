use super::mdns::Mdns;
#[cfg(feature = "esp32")]
use crate::esp32::exec::Esp32Executor;
#[cfg(feature = "native")]
use crate::native::exec::NativeExecutor;
use crate::{
    common::{
        app_client::{AppClient, AppClientBuilder, AppClientConfig},
        grpc::{GrpcBody, GrpcServer},
        grpc_client::GrpcClient,
        robot::LocalRobot,
        webrtc::{
            api::WebRtcApi,
            certificate::Certificate,
            dtls::{DtlsBuilder, DtlsConnector},
            exec::WebRtcExecutor,
            grpc::{WebRtcGrpcBody, WebRtcGrpcServer},
            sctp::Channel,
        },
    },
    proto::{self, app::v1::ConfigResponse},
};
use futures_lite::{
    future::{block_on, Boxed},
    ready, Future,
};
use hyper::server::conn::Http;
use std::{
    error::Error,
    fmt::Debug,
    marker::PhantomData,
    net::Ipv4Addr,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    task::Poll,
};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

#[cfg(feature = "native")]
type Executor<'a> = NativeExecutor<'a>;
#[cfg(feature = "esp32")]
type Executor<'a> = Esp32Executor<'a>;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("couldn't open ssl connection")]
    ServerErrorOpenSslConnection,
    #[error(transparent)]
    Other(#[from] Box<dyn Error + Send + Sync>),
}

pub trait TlsClientConnector {
    type Stream: AsyncRead + AsyncWrite + Unpin + 'static;

    fn connect(&mut self) -> Result<Self::Stream, ServerError>;
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

pub struct ViamServerBuilder<'a, M, CC, L, D, C, T> {
    mdns: M,
    webrtc: Box<WebRtcConfiguration<'a, C, D, CC>>,
    port: u16, // gRPC/HTTP2 port
    http2_listener: L,
    _marker: PhantomData<T>,
    exec: Executor<'a>,
}

impl<'a, M, CC, L, T, D, C> ViamServerBuilder<'a, M, CC, L, D, C, T>
where
    M: Mdns,
    L: AsyncableTcpListener<T>,
    L::Output: Http2Connector<Stream = T>,
    C: TlsClientConnector,
    D: DtlsBuilder,
    CC: Certificate,
    T: AsyncRead + AsyncWrite + Unpin + 'static,
{
    pub fn new(
        mdns: M,
        http2_listener: L,
        webrtc: Box<WebRtcConfiguration<'a, C, D, CC>>,
        exec: Executor<'a>,
        port: u16,
    ) -> Self {
        Self {
            mdns,
            http2_listener,
            port,
            webrtc,
            _marker: PhantomData,
            exec,
        }
    }
    pub fn build(
        mut self,
        config: &ConfigResponse,
    ) -> Result<ViamServer<'a, L, T, C, D, CC>, ServerError> {
        let cfg: RobotCloudConfig = config
            .config
            .as_ref()
            .unwrap()
            .cloud
            .as_ref()
            .unwrap()
            .into();

        self.webrtc.app_config.set_rpc_host(cfg.fqdn.clone());

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

        let srv = ViamServer::new(http2_listener, self.webrtc, cloned_exec);
        Ok(srv)
    }
}

pub trait Http2Connector: std::fmt::Debug {
    type Stream;
    fn accept(&mut self) -> std::io::Result<Self::Stream>;
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

pub struct ViamServer<'a, L, T, C, D, CC> {
    http_listener: HttpListener<L, T>,
    webrtc_config: Box<WebRtcConfiguration<'a, C, D, CC>>,
    exec: Executor<'a>,
}
impl<'a, L, T, C, D, CC> ViamServer<'a, L, T, C, D, CC>
where
    L: AsyncableTcpListener<T>,
    L::Output: Http2Connector<Stream = T>,
    C: TlsClientConnector,
    T: AsyncRead + AsyncWrite + Unpin + 'static,
    CC: Certificate,
    D: DtlsBuilder,
{
    fn new(
        http_listener: HttpListener<L, T>,
        webrtc_config: Box<WebRtcConfiguration<'a, C, D, CC>>,
        exec: Executor<'a>,
    ) -> Self {
        Self {
            http_listener,
            webrtc_config,
            exec,
        }
    }
    pub fn serve_forever(&mut self, robot: Arc<Mutex<LocalRobot>>) {
        let cloned_exec = self.exec.clone();
        block_on(cloned_exec.run(Box::pin(self.serve(robot))));
    }
    async fn serve(&mut self, robot: Arc<Mutex<LocalRobot>>) {
        loop {
            let _ = smol::Timer::after(std::time::Duration::from_millis(100)).await;
            let listener = self.http_listener.next_conn();
            let webrtc = self.webrtc_config.connect_webrtc().await;
            log::info!("waiting for connection");
            let connection = futures_lite::future::or(
                async move {
                    let p = listener.await;
                    p.map(IncomingConnection::Http2Connection)
                        .map_err(|e| ServerError::Other(e.into()))
                },
                async move {
                    let wrt = webrtc.connect().await;
                    wrt.map(IncomingConnection::WebRtcConnection)
                        .map_err(|e| ServerError::Other(e.into()))
                },
            )
            .await;

            if let Err(e) = connection {
                log::error!("error {} while listening", e);
                continue;
            }
            if let Err(e) = match connection.unwrap() {
                IncomingConnection::Http2Connection(c) => self.serve_http2(c, robot.clone()).await,

                IncomingConnection::WebRtcConnection(c) => {
                    self.serve_webrtc(c, robot.clone()).await
                }
            } {
                log::error!("error while serving {}", e);
            }
        }
    }
    async fn serve_http2<U>(
        &self,
        mut c: U,
        robot: Arc<Mutex<LocalRobot>>,
    ) -> Result<(), ServerError>
    where
        U: Http2Connector<Stream = T>,
    {
        let srv = GrpcServer::new(robot.clone(), GrpcBody::new());
        let connection = c.accept().map_err(|e| ServerError::Other(e.into()))?;
        Http::new()
            .with_executor(self.exec.clone())
            .http2_max_concurrent_streams(1)
            .serve_connection(connection, srv)
            .await
            .map_err(|e| ServerError::Other(e.into()))
    }

    async fn serve_webrtc(
        &self,
        c: WebRtcConnector<'a, CC, D::Output, Executor<'a>>,
        robot: Arc<Mutex<LocalRobot>>,
    ) -> Result<(), ServerError> {
        let ret = {
            let channel = c
                .open_data_channel()
                .await
                .map_err(|e| ServerError::Other(e.into()))?;

            let mut grpc = WebRtcGrpcServer::new(
                channel.0,
                GrpcServer::new(robot.clone(), WebRtcGrpcBody::default()),
            );
            loop {
                if let Err(e) = grpc.next_request().await {
                    break Err(ServerError::Other(e.into()));
                }
            }
        };
        let _ = smol::Timer::after(std::time::Duration::from_millis(100)).await;
        ret
    }
}
#[derive(Debug)]
pub enum IncomingConnection<L, U> {
    Http2Connection(L),
    WebRtcConnection(U),
}

pub struct WebRtcConfiguration<'a, C, D, CC> {
    client_connector: C,
    dtls: D,
    cert: Rc<CC>,
    exec: Executor<'a>,
    app_config: AppClientConfig,
}

impl<'a, C, D, CC> WebRtcConfiguration<'a, C, D, CC>
where
    C: TlsClientConnector,
    D: DtlsBuilder,
    CC: Certificate,
{
    pub fn new(
        cert: Rc<CC>,
        dtls: D,
        client_connector: C,
        exec: Executor<'a>,
        app_config: AppClientConfig,
    ) -> Self {
        Self {
            client_connector,
            dtls,
            cert,
            exec,
            app_config,
        }
    }
    async fn connect_webrtc(&mut self) -> WebRtcListener<'a, CC, D::Output, Executor<'a>> {
        let conn = self.client_connector.connect().unwrap();

        let cloned = self.exec.clone();
        let cert = self.cert.clone();
        let grpc_client =
            Box::new(GrpcClient::new(conn, cloned.clone(), "https://app.viam.com:443").unwrap());

        let dtls = self.dtls.make().unwrap();

        let app_client = AppClientBuilder::new(grpc_client, self.app_config.clone())
            .build()
            .unwrap();

        WebRtcListener::new(self.app_config.get_ip(), app_client, cert, dtls, cloned)
    }
}

struct WebRtcConnector<'a, C, D, E> {
    app_client: AppClient<'a>,
    webrtc_api: WebRtcApi<C, D, E>,
}

impl<'a, C, D, E> WebRtcConnector<'a, C, D, E>
where
    C: Certificate,
    D: DtlsConnector,
    E: WebRtcExecutor<Pin<Box<dyn Future<Output = ()> + Send>>> + Clone + 'a,
{
    async fn open_data_channel(self) -> Result<(Channel, WebRtcApi<C, D, E>), ServerError> {
        let mut api = {
            let mut api = self.webrtc_api;
            let _app = self.app_client;
            api.answer()
                .await
                .map_err(|e| ServerError::Other(e.into()))?;
            api.run_ice_until_connected()
                .await
                .map_err(|e| ServerError::Other(e.into()))?;
            api
        };
        let c = api
            .open_data_channel()
            .await
            .map_err(|e| ServerError::Other(e.into()))?;
        Ok((c, api))
    }
}

struct WebRtcListener<'a, C, D, E> {
    ip: Ipv4Addr,
    app_client: AppClient<'a>,
    exec: E,
    cert: Rc<C>,
    dtls: D,
}

impl<'a, C, D, E> WebRtcListener<'a, C, D, E>
where
    C: Certificate,
    D: DtlsConnector,
    E: WebRtcExecutor<Pin<Box<dyn Future<Output = ()> + Send>>> + Clone + 'a,
{
    fn new(ip: Ipv4Addr, app_client: AppClient<'a>, cert: Rc<C>, dtls: D, exec: E) -> Self {
        Self {
            ip,
            app_client,
            cert,
            exec,
            dtls,
        }
    }
    async fn connect(mut self) -> Result<WebRtcConnector<'a, C, D, E>, ServerError> {
        let signaling = self
            .app_client
            .connect_signaling()
            .await
            .map_err(|e| ServerError::Other(e.into()))?;
        let api = WebRtcApi::new(
            self.exec,
            signaling.0,
            signaling.1,
            self.cert,
            self.ip,
            self.dtls,
        );
        Ok(WebRtcConnector {
            app_client: self.app_client,
            webrtc_api: api,
        })
    }
}
