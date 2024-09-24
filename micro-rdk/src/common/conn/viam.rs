use async_channel::{Receiver, RecvError, Sender};
use async_executor::Task;
use async_io::{Async, Timer};
use either::Either;

use futures_lite::{FutureExt, StreamExt};
use futures_util::stream::FuturesUnordered;
use futures_util::TryFutureExt;
use hyper::server::conn::http2;
use hyper::{rt, Uri};
use std::cell::RefCell;
use std::future::Future;

use std::net::{SocketAddr, TcpListener};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;
use std::{fmt::Debug, net::TcpStream};

use crate::common::app_client::{
    AppClient, AppClientBuilder, AppClientError, AppSignaling, PeriodicAppClientTask,
};

use crate::common::config_monitor::ConfigMonitor;
use crate::common::grpc::{GrpcBody, GrpcServer, ServerError};
use crate::common::grpc_client::GrpcClient;
use crate::common::log::LogUploadTask;
use crate::common::provisioning::server::{
    serve_provisioning_async, ProvisioningInfo, WifiManager,
};
use crate::common::registry::ComponentRegistry;
use crate::common::restart_monitor::RestartMonitor;
use crate::common::robot::LocalRobot;
use crate::common::webrtc::api::WebRtcApi;
use crate::common::webrtc::certificate::Certificate;
use crate::common::webrtc::dtls::DtlsBuilder;
use crate::common::{
    credentials_storage::{RobotConfigurationStorage, WifiCredentialStorage},
    exec::Executor,
};
use crate::proto;
use crate::proto::app::v1::RobotConfig;

use super::errors;
use super::mdns::Mdns;
use super::network::Network;
use super::server::{IncomingConnectionManager, WebRTCConnection, WebRtcConfiguration2};

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

pub trait ViamServerStorage:
    RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static
{
}
impl<T> ViamServerStorage for T where
    T: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static
{
}

// Very similar to an Option
// Why not an option, there shouldn't be an operation where taking the inner value is
// valid. Once H2 server is enabled then no way out.
pub(crate) enum HTTP2Server {
    HTTP2Connector(Box<dyn ViamH2Connector>),
    Empty,
}

pub(crate) enum WebRtcListener {
    WebRtc(WebRtcConfiguration2),
    Empty,
}

impl<T: Certificate + ?Sized> Certificate for Box<T> {
    fn get_der_certificate(&self) -> &'_ [u8] {
        (**self).get_der_certificate()
    }
    fn get_der_keypair(&self) -> &'_ [u8] {
        (**self).get_der_keypair()
    }
    fn get_fingerprint(&self) -> &'_ crate::common::webrtc::certificate::Fingerprint {
        (**self).get_fingerprint()
    }
}
impl<T: DtlsBuilder + ?Sized> DtlsBuilder for Box<T> {
    fn make(
        &self,
    ) -> Result<
        Box<dyn crate::common::webrtc::dtls::DtlsConnector>,
        crate::common::webrtc::dtls::DtlsError,
    > {
        (**self).make()
    }
}

pub trait ViamH2Connector {
    // if not called the connection should be opened as PlainText
    fn set_server_certificates(&mut self, srv_cert: Vec<u8>, srv_key: Vec<u8>);
    fn connect_to(
        &self,
        uri: &Uri,
    ) -> Result<std::pin::Pin<Box<dyn IntoHttp2Stream>>, std::io::Error>;
    fn accept_connection(
        &self,
        connection: Async<TcpStream>,
    ) -> Result<std::pin::Pin<Box<dyn IntoHttp2Stream>>, std::io::Error>;
}

pub trait HTTP2Stream: rt::Read + rt::Write + Unpin {}
pub trait IntoHttp2Stream: Future<Output = Result<Box<dyn HTTP2Stream>, std::io::Error>> {}

impl<T> HTTP2Stream for T where T: rt::Read + rt::Write + Unpin {}

pub struct ViamServerBuilder2<Storage> {
    storage: Storage,
    http2_server: HTTP2Server,
    webrtc_configuration: WebRtcListener,
    provisioning_info: ProvisioningInfo,
    wifi_manager: Option<Box<dyn WifiManager>>,
    component_registry: Box<ComponentRegistry>,
    http2_server_port: u16,
    http2_server_insecure: bool,
    initial_app_uri: Uri,
    app_client_tasks: Vec<Box<dyn PeriodicAppClientTask>>,
    max_concurrent_connections: u32,
}

impl<Storage> ViamServerBuilder2<Storage>
where
    Storage: ViamServerStorage,
    <Storage as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<Storage as RobotConfigurationStorage>::Error>,
{
    pub fn new(storage: Storage) -> ViamServerBuilder2<Storage> {
        ViamServerBuilder2 {
            storage,
            http2_server: HTTP2Server::Empty,
            webrtc_configuration: WebRtcListener::Empty,
            provisioning_info: Default::default(),
            wifi_manager: None,
            component_registry: Default::default(),
            http2_server_port: 12346,
            http2_server_insecure: false,
            initial_app_uri: "https://app.viam.com:443".try_into().unwrap(),
            app_client_tasks: Default::default(),
            max_concurrent_connections: 1,
        }
    }
    pub fn with_max_concurrent_connection(&mut self, max_concurrent_connections: u32) -> &mut Self {
        self.max_concurrent_connections = max_concurrent_connections;
        self
    }
    pub fn with_provisioning_info(&mut self, provisioning_info: ProvisioningInfo) -> &mut Self {
        self.provisioning_info = provisioning_info;
        self
    }
    pub fn with_http2_server<H>(&mut self, http2_connector: H, port: u16) -> &mut Self
    where
        H: ViamH2Connector + 'static,
    {
        self.http2_server = HTTP2Server::HTTP2Connector(Box::new(http2_connector));
        self.http2_server_port = port;
        self
    }
    pub fn with_http2_server_insecure(&mut self, insecure: bool) -> &mut Self {
        self.http2_server_insecure = insecure;
        self
    }
    pub fn with_app_uri(&mut self, uri: Uri) -> &mut Self {
        self.initial_app_uri = uri;
        self
    }
    pub fn with_webrtc_configuration(
        &mut self,
        webrtc_configuration: WebRtcConfiguration2,
    ) -> &mut Self {
        self.webrtc_configuration = WebRtcListener::WebRtc(webrtc_configuration);
        self
    }
    pub fn wifi_wifi_manager(
        &mut self,
        wifi_manager: impl Into<Option<Box<dyn WifiManager>>>,
    ) -> &mut Self {
        self.wifi_manager = wifi_manager.into();
        self
    }
    pub fn with_component_registry(
        &mut self,
        component_registry: Box<ComponentRegistry>,
    ) -> &mut Self {
        self.component_registry = component_registry;
        self
    }
    pub fn with_app_client_task(&mut self, task: Box<dyn PeriodicAppClientTask>) -> &mut Self {
        self.app_client_tasks.push(task);
        self
    }
    pub fn with_default_tasks(&mut self) -> &mut Self {
        let restart_monitor = Box::new(RestartMonitor::new(|| std::process::exit(0)));
        let log_upload = Box::new(LogUploadTask);
        self.with_app_client_task(restart_monitor)
            .with_app_client_task(log_upload);
        self
    }
    pub fn build<C, M>(
        self,
        http2_connector: C,
        executor: Executor,
        mdns: M,
        network: Box<dyn Network>,
    ) -> ViamServer2<Storage, C, M>
    where
        C: ViamH2Connector + 'static,
        M: Mdns,
    {
        ViamServer2 {
            executor,
            storage: self.storage,
            http2_server: self.http2_server,
            webrtc_configuration: self.webrtc_configuration,
            http2_connector,
            mdns: RefCell::new(mdns),
            component_registry: self.component_registry,
            provisioning_info: self.provisioning_info,
            http2_server_insecure: self.http2_server_insecure,
            http2_server_port: self.http2_server_port,
            app_uri: self.initial_app_uri,
            wifi_manager: Rc::new(self.wifi_manager),
            app_client_tasks: self.app_client_tasks,
            max_concurrent_connections: self.max_concurrent_connections,
            network,
        }
    }
}
pub struct ViamServer2<Storage, C, M> {
    executor: Executor,
    storage: Storage,
    http2_server: HTTP2Server,
    webrtc_configuration: WebRtcListener,
    http2_connector: C,
    provisioning_info: ProvisioningInfo,
    mdns: RefCell<M>,
    component_registry: Box<ComponentRegistry>,
    http2_server_insecure: bool,
    http2_server_port: u16,
    app_uri: Uri,
    wifi_manager: Rc<Option<Box<dyn WifiManager>>>,
    app_client_tasks: Vec<Box<dyn PeriodicAppClientTask>>,
    max_concurrent_connections: u32,
    network: Box<dyn Network>,
}

impl<Storage, C, M> ViamServer2<Storage, C, M>
where
    Storage: ViamServerStorage,
    <Storage as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<Storage as RobotConfigurationStorage>::Error>,
    C: ViamH2Connector + 'static,
    M: Mdns,
{
    pub fn run_forever(&mut self) {
        let exec = self.executor.clone();
        exec.block_on(self.run());
    }
    pub(crate) async fn run(&mut self) {
        // The first step is to check whether or not credentials are populated in
        // storage. If not, we should go straight to provisioning.
        if !self.storage.has_robot_credentials() {
            self.provision().await;
        }

        let robot_creds = self.storage.get_robot_credentials().unwrap();
        // assume credentials are valid for now
        // attempt to instantiate an app client
        // if we have an unauthenticated or permission denied error, we erase the creds
        // and restart
        // otherwise we assume some network layer error and attempt to start the robot from cached
        // data
        let app_client = self
            .connect_to_app()
            .await
            .inspect_err(|error| {
                if error.is_permission_denied() || error.is_unauthenticated() {
                    let _ = self.storage.reset_robot_credentials().inspect_err(|err| {
                        log::error!("error {:?} while erasing credentials", err)
                    });
                    let _ = self.storage.reset_robot_configuration().inspect_err(|err| {
                        log::error!("error {:?} while erasing configuration", err)
                    });
                    #[cfg(not(test))]
                    panic!("erased credentials restart robot"); // TODO bubble up error and go back in provisioning
                }
                log::error!("couldn't connect to {} reason {:?}", self.app_uri, error);
            })
            .ok();

        // The next step is to build the robot based on the config retrieved online or from storage. Defaulting to an empty
        // robot if neither are available
        // If we are offline viam server will not start webrtc listening (AppClient wil not be constructed)
        // However we are still able to connect locally (H2) and we should cache data if the data manager exists.
        // is_connected only tells us whether or not we are on a network
        let config = match app_client.as_ref() {
            Some(app) => app
                .get_app_config(Some(self.network.get_ip()))
                .await
                .inspect_err(|err| {
                    log::error!(
                        "couldn't get config, will default to cached config reason {:?}",
                        err
                    )
                })
                .ok(),
            None => None,
        };
        let (config, build_time) = config.map_or_else(
            || {
                (
                    self.storage
                        .get_robot_configuration()
                        .ok() //can inspect and report empty robot will be constructed
                        .map_or(Box::default(), Box::new),
                    None,
                )
            },
            |resp| (resp.0.config.map_or(Box::default(), Box::new), resp.1),
        );

        if let Err(err) = self.storage.store_robot_configuration(&config) {
            log::error!("couldn't store the robot configuration reason {:?}", err);
        }

        let config_monitor_task = Box::new(ConfigMonitor::new(
            config.clone(),
            self.storage.clone(),
            || std::process::exit(0),
        ));
        self.app_client_tasks.push(config_monitor_task);

        let mut robot = LocalRobot::from_cloud_config2(
            self.executor.clone(),
            robot_creds.robot_id.clone(),
            &config,
            &mut self.component_registry,
            build_time,
        )
        .inspect_err(|err| log::error!("couldn't build the robot reason {:?}", err))
        .unwrap_or_default();

        self.app_client_tasks
            .append(&mut robot.get_periodic_app_client_tasks());

        let robot = Arc::new(Mutex::new(robot));

        if let HTTP2Server::HTTP2Connector(s) = &mut self.http2_server {
            if !self.http2_server_insecure {
                // TODO implement certificate storage
                let certs = app_client
                    .as_ref()
                    .unwrap()
                    .get_certificates()
                    .await
                    .unwrap();
                s.set_server_certificates(
                    certs.tls_certificate.into_bytes(),
                    certs.tls_private_key.into_bytes(),
                );
            }
        }

        let (tx, rx) = async_channel::bounded(1);
        let mut inner = RobotServer {
            http2_server: &self.http2_server,
            http2_server_port: self.http2_server_port,
            executor: self.executor.clone(),
            robot: robot.clone(),
            mdns: &self.mdns,
            webrtc_signaling: rx,
            webrtc_config: &self.webrtc_configuration,
            network: &self.network,
            incommin_connection_manager: IncomingConnectionManager::new(
                self.max_concurrent_connections as usize,
            ),
            robot_config: &config,
        };

        if let Some(cfg) = config.cloud.as_ref() {
            self.app_client_tasks.push(Box::new(SignalingTask {
                sender: tx,
                rpc_host: cfg.fqdn.clone(),
            }));
        }

        let mut tasks: FuturesUnordered<_> = FuturesUnordered::new();
        tasks.push(Either::Right(self.run_app_client_tasks(app_client)));
        tasks.push(Either::Left(inner.run()));
        while let Some(ret) = tasks.next().await {
            log::error!("task ran returned {:?}", ret);
        }
    }
    async fn connect_to_app(&self) -> Result<AppClient, AppClientError> {
        //ugly hack to remove last /
        //needs to change that but would need to update GrpcClient
        let uri = self.app_uri.to_string();
        let mut chars = uri.chars();
        chars.next_back();
        let uri = chars.as_str();
        let robot_creds = self.storage.get_robot_credentials().unwrap();
        let app_client_io = self
            .http2_connector
            .connect_to(&self.app_uri)
            .map_err(AppClientError::AppClientIoError)?
            .await
            .map_err(AppClientError::AppClientIoError)?;
        let grpc_client = GrpcClient::new(app_client_io, self.executor.clone(), uri)
            .await
            .map_err(AppClientError::AppGrpcClientError)?;

        AppClientBuilder::new(Box::new(grpc_client), robot_creds.clone())
            .build()
            .await
    }
    // run task forever reconnecting on demand
    // if a task returns an error, the app client will be dropped
    async fn run_app_client_tasks(
        &self,
        app_client: Option<AppClient>,
    ) -> Result<(), errors::ServerError> {
        let mut app_client = app_client;
        let wait = Duration::from_secs(1); // should do exponential back off
        loop {
            if let Some(app_client) = app_client {
                let mut app_client_tasks: FuturesUnordered<AppClientTaskRunner> =
                    FuturesUnordered::new();
                for task in &self.app_client_tasks {
                    app_client_tasks.push(AppClientTaskRunner {
                        app_client: &app_client,
                        invoker: task,
                        state: TaskRunnerState::Run {
                            task: task.invoke(&app_client),
                        },
                    });
                }
                while let Some(res) = app_client_tasks.next().await {
                    if let Err(err) = res {
                        log::error!("a task returned the following error {:?}", err);
                        break;
                    }
                }
            }
            // the only way to reach here is either we had a None passed (app_client wasn't connected at boot)
            // or an error was reported by an underlying task which means that app client
            // is considered gone
            let _ = Timer::after(wait).await;
            app_client = self
                .connect_to_app()
                .await
                .inspect_err(|error| {
                    if error.is_permission_denied() || error.is_unauthenticated() {
                        let _ = self.storage.reset_robot_credentials().inspect_err(|err| {
                            log::error!("error {:?} while erasing credentials", err)
                        });
                        let _ = self.storage.reset_robot_configuration().inspect_err(|err| {
                            log::error!("error {:?} while erasing configuration", err)
                        });
                        #[cfg(not(test))]
                        panic!("erased credentials restart robot"); // TODO bubble up error and go back in provisioning
                    }
                    log::error!("couldn't connect to {} reason {:?}", self.app_uri, error);
                })
                .ok();
        }
    }
    // I am adding provisioning in the main flow of viamserver
    // this is however outside of the scope IMO. What could be a better way?
    // We don't want the user to have to write code to handle the provisioning
    // case.
    async fn provision(&self) {
        let mut last_error = None;

        while let Err(e) = serve_provisioning_async(
            self.executor.clone(),
            Some(self.provisioning_info.clone()),
            self.storage.clone(),
            last_error.take(),
            self.wifi_manager.clone(),
            &self.mdns,
        )
        .await
        {
            let _ = last_error.insert(e);
        }
    }
}

// The RobotServer aims to serve local connection so it exists independently from
// AppClient. It will need to be recreated when either when the ip changes. or if
// the robot config changes
// For now can only exists when network returns an IP
// WebRTC should be handled here to with the caveat that Signaling should be made
// through a Pipe. So the "hacky" thing we are doing is using a Receiver getting
// AppSignaling Objects that we can await
struct RobotServer<'a, M> {
    http2_server: &'a HTTP2Server,
    webrtc_config: &'a WebRtcListener,
    executor: Executor,
    robot: Arc<Mutex<LocalRobot>>,
    mdns: &'a RefCell<M>,
    http2_server_port: u16,
    webrtc_signaling: Receiver<AppSignaling>,
    network: &'a dyn Network,
    incommin_connection_manager: IncomingConnectionManager,
    robot_config: &'a RobotConfig,
}

pub(crate) enum IncomingConnection2 {
    HTTP2Connection(std::io::Result<(Async<TcpStream>, SocketAddr)>),
    WebRTCConnection(Result<AppSignaling, RecvError>),
}

impl<'a, M> RobotServer<'a, M>
where
    M: Mdns,
{
    async fn run(&mut self) -> Result<(), errors::ServerError> {
        let http2_listener = if let HTTP2Server::HTTP2Connector(_) = self.http2_server {
            if let Some(cfg) = self.robot_config.cloud.as_ref() {
                let mut mdns = self.mdns.borrow_mut();
                let cfg: RobotCloudConfig = cfg.into();
                mdns.set_hostname(&cfg.name)
                    .map_err(|e| errors::ServerError::Other(e.into()))?;
                mdns.add_service(
                    &cfg.local_fqdn.replace('.', "-"),
                    "_rpc",
                    "_tcp",
                    self.http2_server_port,
                    &[("grpc", "")],
                )
                .map_err(|e| errors::ServerError::Other(e.into()))?;
                mdns.add_service(
                    &cfg.fqdn.replace('.', "-"),
                    "_rpc",
                    "_tcp",
                    self.http2_server_port,
                    &[("grpc", "")],
                )
                .map_err(|e| errors::ServerError::Other(e.into()))?;
            }
            Some(async_io::Async::new(TcpListener::bind(format!(
                "0.0.0.0:{}",
                self.http2_server_port
            ))?)?)
        } else {
            None
        };

        loop {
            let h2_conn: Pin<Box<dyn Future<Output = IncomingConnection2>>> =
                if let HTTP2Server::Empty = self.http2_server {
                    Box::pin(async {
                        IncomingConnection2::HTTP2Connection(futures_lite::future::pending().await)
                    })
                } else {
                    // safe to unwrap, always exists
                    Box::pin(async {
                        IncomingConnection2::HTTP2Connection(
                            http2_listener.as_ref().unwrap().accept().await,
                        )
                    })
                };

            let webrtc_conn: Pin<Box<dyn Future<Output = IncomingConnection2>>> =
                if let WebRtcListener::Empty = self.webrtc_config {
                    Box::pin(async {
                        IncomingConnection2::HTTP2Connection(futures_lite::future::pending().await)
                    })
                } else {
                    Box::pin(async {
                        IncomingConnection2::WebRTCConnection(self.webrtc_signaling.recv().await)
                    })
                };

            let r = futures_lite::future::or(h2_conn, webrtc_conn);
            // TODO consider moving these errors out of the run fn
            // in practice we may want to recover in case of failure to not disrupt existing connections
            match r.await {
                IncomingConnection2::HTTP2Connection(conn) => {
                    if let HTTP2Server::HTTP2Connector(h) = self.http2_server {
                        if self.incommin_connection_manager.get_lowest_prio() < u32::MAX {
                            let stream = conn?;
                            let io = h.accept_connection(stream.0)?.await?;
                            let task = self.server_peer_http2(io);
                            self.incommin_connection_manager
                                .insert_new_conn(task, u32::MAX)
                                .await;
                        }
                    }
                }
                IncomingConnection2::WebRTCConnection(conn) => {
                    let sig = conn.map_err(|e| errors::ServerError::Other(e.into()))?;
                    let ip = self.network.get_ip();
                    if let WebRtcListener::WebRtc(conf) = self.webrtc_config {
                        let mut api = WebRtcApi::new(
                            self.executor.clone(),
                            sig.0,
                            sig.1,
                            conf.cert.clone(),
                            ip,
                            conf.dtls.make()?,
                        );
                        let sdp = api.answer(0).await?;
                        let mut c = WebRTCConnection {
                            webrtc_api: api,
                            sdp: sdp.0,
                            server: None,
                            robot: self.robot.clone(),
                            prio: sdp.1,
                        };
                        c.open_data_channel().await?;
                        let prio = c.prio;
                        let task = self.executor.spawn(async move { c.run().await });
                        self.incommin_connection_manager
                            .insert_new_conn(task, prio)
                            .await;
                    }
                }
            }
        }
    }
    fn server_peer_http2(&self, io: Box<dyn HTTP2Stream>) -> Task<Result<(), errors::ServerError>> {
        let robot = self.robot.clone();
        let exec = self.executor.clone();
        let srv = GrpcServer::new(robot, GrpcBody::new());
        let srv = Box::new(
            http2::Builder::new(exec)
                .initial_connection_window_size(2048)
                .initial_stream_window_size(2048)
                .max_send_buf_size(4096)
                .max_concurrent_streams(2)
                .serve_connection(io, srv),
        );
        self.executor
            .spawn(srv.map_err(|e| errors::ServerError::Other(e.into())))
    }
}

struct SignalingTask {
    sender: Sender<AppSignaling>,
    rpc_host: String,
}

impl PeriodicAppClientTask for SignalingTask {
    fn get_default_period(&self) -> Duration {
        Duration::from_secs(0)
    }
    fn name(&self) -> &str {
        "SignalingTask"
    }
    fn invoke<'b, 'a: 'b>(
        &'a self,
        app_client: &'b AppClient,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'b>>
    {
        Box::pin(async {
            let sig_pair = app_client.initiate_signaling(self.rpc_host.clone()).await?;
            let _ = self.sender.send(sig_pair).await; // TODO deal with result, sending on a close channel will never succeed. The limit here is that SignalingTask will be allocated for the lifetime of the ViamServer.
            Ok(None)
        })
    }
}

pin_project_lite::pin_project! {
    #[project = TaskRunnerStateProj]
    enum TaskRunnerState<'a> {
    Sleep{#[pin]timer : Timer},
    Run{ task: std::pin::Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'a>>},
    }
}

impl<'a> Future for TaskRunnerState<'a> {
    type Output = Result<Option<Duration>, AppClientError>;
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            TaskRunnerStateProj::Run { task } => {
                let res = futures_lite::ready!(task.poll(cx));
                Poll::Ready(res)
            }
            TaskRunnerStateProj::Sleep { timer } => {
                let _ = futures_lite::ready!(timer.poll(cx));
                Poll::Ready(Ok(None))
            }
        }
    }
}

pin_project_lite::pin_project! {
    struct AppClientTaskRunner<'a> {
    invoker: &'a dyn PeriodicAppClientTask, //need to impl deref?
    app_client: &'a AppClient,
    #[pin]
    state: TaskRunnerState<'a>
    }
}

impl<'a> Future for AppClientTaskRunner<'a> {
    type Output = Result<(), AppClientError>;
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let res = {
            let this = self.as_mut().project();
            futures_lite::ready!(this.state.poll(cx))?
        };
        // we need to swap the state between Run,Sleep such as Run -> Sleep or Sleep -> Run
        // it's not possible in safe rust to mutate the inner state therefore we need to resort to
        // unsafe code
        unsafe {
            // move self.state out of self, from this point on self.state is in an invalid state
            // because we have it pinned there are no risk of another part of the code reading this field
            // however if a panic occurs while mutating the state this will lead to UB since
            // dropping TaskRunner will be invalid
            // To circumvent this we catch panic as they happen (either when calling self.invoker.invoke() or instantiating
            // the new timer.
            // If a panic occurs and abort call will be issued. We could return an error but we would need to either write the value
            // moved self.state back or put a default value.
            let old = std::ptr::read(&self.state);
            let next = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match old {
                TaskRunnerState::Run { task: _ } => TaskRunnerState::Sleep {
                    timer: res.map_or(
                        Timer::after(self.invoker.get_default_period()),
                        Timer::after,
                    ),
                },
                TaskRunnerState::Sleep { timer: _ } => TaskRunnerState::Run {
                    task: self.invoker.invoke(self.app_client),
                },
            }))
            .unwrap_or_else(|_| std::process::abort());
            // move the new value into self.state, the old value will be dropped when leaving the unsafe block
            std::ptr::write(&mut self.state, next);
        }
        // state has changed we need to poll again immediately
        cx.waker().wake_by_ref();

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
        pin::Pin,
        rc::Rc,
        sync::{
            atomic::{AtomicI32, Ordering},
            Arc,
        },
        time::Duration,
    };

    use crate::{
        common::{
            app_client::encode_request,
            conn::{
                network::{ExternallyManagedNetwork, Network},
                server::WebRtcConfiguration2,
                viam::ViamServerBuilder2,
            },
            credentials_storage::{RAMStorage, RobotConfigurationStorage},
            exec::Executor,
            grpc::{GrpcBody, GrpcError, GrpcResponse, ServerError},
            log::LogUploadTask,
            provisioning::server::ProvisioningInfo,
            restart_monitor::RestartMonitor,
            webrtc::certificate::Certificate,
        },
        native::{
            certificate::WebRtcCertificate,
            conn::mdns::NativeMdns,
            dtls::NativeDtls,
            tcp::{NativeH2Connector, NativeStream},
        },
        proto::{
            app::{
                self,
                v1::{
                    CertificateResponse, ConfigResponse, NeedsRestartRequest, NeedsRestartResponse,
                    RobotConfig,
                },
            },
            provisioning::v1::{CloudConfig, SetSmartMachineCredentialsRequest},
            robot::v1::{LogRequest, LogResponse, ResourceNamesRequest},
            rpc::v1::{AuthenticateRequest, AuthenticateResponse},
        },
        tests::global_network_test_lock,
    };
    use async_executor::Task;
    use async_io::{Async, Timer};
    use bytes::{BufMut, Bytes, BytesMut};
    use futures_lite::FutureExt;
    use http_body_util::{BodyExt, Full};
    use hyper::{
        body::Incoming,
        header::{CONTENT_TYPE, TE},
        server::conn::http2,
        service::Service,
        Method,
    };
    use mdns_sd::{ServiceEvent, ServiceInfo};
    use prost::Message;
    use rustls::client::ServerCertVerifier;

    #[derive(Clone, Default)]
    struct AppServerInsecure {
        config_fn: Option<Rc<Box<dyn Fn() -> RobotConfig>>>,
        log_fn: Option<&'static dyn Fn()>,
        auth_fn: Option<Rc<Box<dyn Fn(&AuthenticateRequest) -> bool>>>,
    }

    impl AppServerInsecure {
        fn authenticate(&self, body: Bytes) -> Result<Bytes, ServerError> {
            let req = AuthenticateRequest::decode(body).unwrap();
            if let Some(auth_fn) = &self.auth_fn {
                if !auth_fn(&req) {
                    return Err(ServerError::new(GrpcError::RpcPermissionDenied, None));
                }
            }
            let resp = AuthenticateResponse {
                access_token: "fake".to_string(),
            };
            let len = resp.encoded_len();
            let mut buffer = BytesMut::with_capacity(5 + len);
            buffer.put_u8(0);
            buffer.put_u32(len.try_into().unwrap());
            resp.encode(&mut buffer).unwrap();
            Ok(buffer.freeze())
        }
        fn log(&self, body: Bytes) -> Bytes {
            let _req = LogRequest::decode(body).unwrap();
            if let Some(log_fn) = self.log_fn.as_ref() {
                log_fn();
            }
            let resp = LogResponse::default();
            let len = resp.encoded_len();
            let mut buffer = BytesMut::with_capacity(5 + len);
            buffer.put_u8(0);
            buffer.put_u32(len.try_into().unwrap());
            resp.encode(&mut buffer).unwrap();
            buffer.freeze()
        }
        fn needs_restart(&self, body: Bytes) -> Bytes {
            let _req = NeedsRestartRequest::decode(body).unwrap();

            let resp = NeedsRestartResponse {
                id: "".to_string(),
                must_restart: false,
                ..Default::default()
            };
            let len = resp.encoded_len();
            let mut buffer = BytesMut::with_capacity(5 + len);
            buffer.put_u8(0);
            buffer.put_u32(len.try_into().unwrap());
            resp.encode(&mut buffer).unwrap();
            buffer.freeze()
        }
        fn certificates(&self, _body: Bytes) -> Bytes {
            let self_signed =
                rcgen::generate_simple_self_signed(["localhost".to_string()]).unwrap();
            let tls_certificate = self_signed.serialize_pem().unwrap();
            let tls_private_key = self_signed.serialize_private_key_pem();
            let resp = CertificateResponse {
                id: "".to_owned(),
                tls_certificate,
                tls_private_key,
            };
            let len = resp.encoded_len();
            let mut buffer = BytesMut::with_capacity(5 + len);
            buffer.put_u8(0);
            buffer.put_u32(len.try_into().unwrap());
            resp.encode(&mut buffer).unwrap();
            buffer.freeze()
        }
        fn get_config(&self) -> Bytes {
            let cfg = self
                .config_fn
                .as_ref()
                .map_or(make_sample_config(), |cfg_fn| cfg_fn());
            let resp = ConfigResponse { config: Some(cfg) };
            let len = resp.encoded_len();
            let mut buffer = BytesMut::with_capacity(5 + len);
            buffer.put_u8(0);
            buffer.put_u32(len.try_into().unwrap());
            resp.encode(&mut buffer).unwrap();
            buffer.freeze()
        }
        async fn process_request_inner(
            &self,
            req: hyper::http::Request<Incoming>,
        ) -> Result<Bytes, ServerError> {
            let (parts, body) = req.into_parts();
            let mut body = body
                .collect()
                .await
                .map_err(|_| GrpcError::RpcFailedPrecondition)?
                .to_bytes();
            let out = match parts.uri.path() {
                "/proto.rpc.v1.AuthService/Authenticate" => self.authenticate(body.split_off(5))?,
                "/viam.app.v1.RobotService/Certificate" => self.certificates(body.split_off(5)),
                "/viam.app.v1.RobotService/Log" => self.log(body.split_off(5)),
                "/viam.app.v1.RobotService/NeedsRestart" => self.needs_restart(body.split_off(5)),
                "/viam.app.v1.RobotService/Config" => self.get_config(),
                _ => panic!("unsupported uri {:?}", parts.uri.path()),
            };
            Ok(out)
        }
        async fn process_request(
            &self,
            req: hyper::http::Request<Incoming>,
        ) -> Result<
            hyper::http::Response<GrpcBody>,
            Box<dyn std::error::Error + Send + Sync + 'static>,
        > {
            let mut resp = GrpcBody::new();
            match self.process_request_inner(req).await {
                Ok(bytes) => resp.put_data(bytes),
                Err(e) => resp.set_status(e.status_code(), Some(e.to_string())),
            };
            hyper::http::Response::builder()
                .status(200)
                .header(CONTENT_TYPE, "application/grpc")
                .body(resp)
                .map_err(|e| e.into())
        }
    }
    impl Service<hyper::http::Request<Incoming>> for AppServerInsecure {
        type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
        type Response = hyper::http::Response<GrpcBody>;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
        fn call(&self, req: hyper::http::Request<Incoming>) -> Self::Future {
            let svc = self.clone();

            Box::pin(async move { svc.process_request(req).await })
        }
    }

    #[derive(Debug)]
    struct InsecureCertAcceptor;
    impl ServerCertVerifier for InsecureCertAcceptor {
        fn verify_server_cert(
            &self,
            _: &rustls::Certificate,
            _: &[rustls::Certificate],
            _: &rustls::ServerName,
            _: &mut dyn Iterator<Item = &[u8]>,
            _: &[u8],
            _: std::time::SystemTime,
        ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
            // always return  yes, we **may** want to validate the generated cert for the sake
            // of it. But considering we are running tests might not be needed.
            Ok(rustls::client::ServerCertVerified::assertion())
        }
    }

    #[test_log::test]
    fn test_app_permission_denied() {
        let _unused = global_network_test_lock();
        let ram_storage = RAMStorage::new();
        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };

        let creds = CloudConfig {
            id: "test-denied".to_string(),
            secret: "".to_string(),
            app_address: "".to_owned(),
        };
        assert!(ram_storage.store_robot_credentials(creds).is_ok());

        let mdns = NativeMdns::new("".to_owned(), network.get_ip());
        assert!(mdns.is_ok());
        let mdns = mdns.unwrap();
        let cloned_ram_storage = ram_storage.clone();
        let mut viam_server = ViamServerBuilder2::new(ram_storage);
        viam_server
            .with_app_uri("http://localhost:56563".try_into().unwrap())
            .with_http2_server(NativeH2Connector::default(), 12346)
            .with_max_concurrent_connection(2)
            .with_http2_server_insecure(true)
            .with_default_tasks();

        let exec = Executor::new();

        let mut viam_server = viam_server.build(
            NativeH2Connector::default(),
            exec.clone(),
            mdns,
            Box::new(network),
        );
        let cloned_exec = exec.clone();

        let mut app = AppServerInsecure::default();

        app.auth_fn = Some(Rc::new(Box::new(move |req| {
            assert!(req.entity.contains("test-denied"));
            false
        })));
        exec.block_on(async move {
            let other_clone = cloned_exec.clone();
            let _fake_server_task =
                cloned_exec.spawn(async move { run_fake_app_server(other_clone, app).await });
            let _task = cloned_exec.spawn(async move {
                viam_server.run().await;
            });
            let _ = Timer::after(Duration::from_millis(500)).await;
            assert!(!cloned_ram_storage.has_robot_credentials())
        });
    }

    #[test_log::test]
    // The goal of the test is to confirm that transient failure of the app client caused
    // by network issues (and not permission issues)
    // an http2 connection to should remain valid for the lifetime of the test
    fn test_app_client_transient_failure() {
        let _unused = global_network_test_lock();
        let ram_storage = RAMStorage::new();
        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };

        let creds = CloudConfig {
            id: "test-transient".to_string(),
            secret: "".to_string(),
            app_address: "".to_owned(),
        };
        assert!(ram_storage.store_robot_credentials(creds).is_ok());

        let mdns = NativeMdns::new("".to_owned(), network.get_ip());
        assert!(mdns.is_ok());
        let mdns = mdns.unwrap();

        let mut viam_server = ViamServerBuilder2::new(ram_storage);
        viam_server
            .with_app_uri("http://localhost:56563".try_into().unwrap())
            .with_http2_server(NativeH2Connector::default(), 12346)
            .with_max_concurrent_connection(2)
            .with_default_tasks();

        let exec = Executor::new();

        let mut viam_server = viam_server.build(
            NativeH2Connector::default(),
            exec.clone(),
            mdns,
            Box::new(network),
        );
        let cloned_exec = exec.clone();

        let mut app = AppServerInsecure::default();
        let shared_auth_counter = Rc::new(AtomicI32::new(0));
        let shared_auth_counter_cloned = shared_auth_counter.clone();
        app.auth_fn = Some(Rc::new(Box::new(move |req| {
            assert!(req.entity.contains("test-transient"));
            let _ = shared_auth_counter_cloned.fetch_add(1, Ordering::AcqRel);
            true
        })));
        app.config_fn = Some(Rc::new(Box::new(|| {
            let mut cfg = make_sample_config();
            if let Some(cloud) = cfg.cloud.as_mut() {
                cloud.fqdn = "test-bot.xxds65ui.viam.cloud".to_owned();
                cloud.local_fqdn = "test-bot.xxds65ui.viam.local.cloud".to_owned();
            }
            cfg
        })));

        let cloned_app = app.clone();

        exec.block_on(async move {
            let other_clone = cloned_exec.clone();
            let fake_server_task =
                cloned_exec.spawn(async move { run_fake_app_server(other_clone, app).await });
            let _task = cloned_exec.spawn(async move {
                viam_server.run().await;
            });
            let record = look_for_an_mdns_record("_rpc._tcp.local.", "grpc", "test-bot")
                .or(async {
                    let _ = Timer::after(Duration::from_secs(1)).await;
                    Err("timeout".into())
                })
                .await;

            assert!(record.is_ok());
            let record = record.unwrap();

            let addr = record.get_addresses_v4().into_iter().take(1).next();
            assert!(addr.is_some());
            let addr = addr.unwrap();
            let port = record.get_port();
            let addr = SocketAddr::new(std::net::IpAddr::V4(*addr), port);

            let t1 = test_connect_to(addr, cloned_exec.clone()).await;
            assert!(t1.is_ok());

            // one call to authenticate
            assert_eq!(shared_auth_counter.load(Ordering::Acquire), 1);

            // cancel the fake app task
            // this should simulate a network loss or app.viam.com going offline for some
            // reasons. We should still be able to make a connection to the H2 server
            assert!(fake_server_task.cancel().await.is_none());
            let _ = Timer::after(Duration::from_millis(300));
            let t2 = test_connect_to(addr, cloned_exec.clone()).await;
            assert!(t2.is_ok());
            assert_eq!(shared_auth_counter.load(Ordering::Acquire), 1);

            let other_clone = cloned_exec.clone();
            // bring up a new app client to simulate network resuming.
            // we just check that another call to authenticate has been issues
            // this indicate that a connection was made.
            let _fake_server_task = cloned_exec
                .spawn(async move { run_fake_app_server(other_clone, cloned_app).await });
            let _ = Timer::after(Duration::from_secs(2)).await;
            assert_eq!(shared_auth_counter.load(Ordering::Acquire), 2);
        });
    }

    #[test_log::test]
    /// Runs viam server exposing HTTP2 connections, since each HTTP2 connection gets a
    /// max_prio assigned we can't test preemption
    /// Testing webrtc would require to add support for ice control agent931
    fn test_multiple_connection_http2() {
        let _unused = global_network_test_lock();
        let ram_storage = RAMStorage::new();
        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };

        let creds = CloudConfig {
            id: "".to_string(),
            secret: "".to_string(),
            app_address: "".to_owned(),
        };

        assert!(ram_storage.store_robot_credentials(creds).is_ok());

        let mdns = NativeMdns::new("".to_owned(), network.get_ip());
        assert!(mdns.is_ok());
        let mdns = mdns.unwrap();

        let mut viam_server = ViamServerBuilder2::new(ram_storage);
        viam_server
            .with_app_uri("http://localhost:56563".try_into().unwrap())
            .with_http2_server(NativeH2Connector::default(), 12346)
            .with_max_concurrent_connection(3);

        let exec = Executor::new();

        let mut viam_server = viam_server.build(
            NativeH2Connector::default(),
            exec.clone(),
            mdns,
            Box::new(network),
        );
        let cloned_exec = exec.clone();

        let mut app = AppServerInsecure::default();
        app.config_fn = Some(Rc::new(Box::new(|| {
            let mut cfg = make_sample_config();
            if let Some(cloud) = cfg.cloud.as_mut() {
                cloud.fqdn = "test-bot.xxds65ui.viam.cloud".to_owned();
                cloud.local_fqdn = "test-bot.xxds65ui.viam.local.cloud".to_owned();
            }
            cfg
        })));

        let _fake_server_task =
            exec.spawn(async move { run_fake_app_server(cloned_exec, app).await });

        let cloned_exec = exec.clone();
        exec.block_on(async move {
            let _task = cloned_exec.spawn(async move {
                viam_server.run().await;
            });
            let record = look_for_an_mdns_record("_rpc._tcp.local.", "grpc", "test-bot")
                .or(async {
                    let _ = Timer::after(Duration::from_secs(1)).await;
                    Err("timeout".into())
                })
                .await;

            assert!(record.is_ok());
            let record = record.unwrap();

            let addr = record.get_addresses_v4().into_iter().take(1).next();
            assert!(addr.is_some());
            let addr = addr.unwrap();
            let port = record.get_port();
            let addr = SocketAddr::new(std::net::IpAddr::V4(*addr), port);

            let t1 = test_connect_to(addr, cloned_exec.clone()).await;
            assert!(t1.is_ok());

            let t2 = test_connect_to(addr, cloned_exec.clone()).await;
            assert!(t2.is_ok());

            let t3 = test_connect_to(addr, cloned_exec.clone()).await;
            assert!(t3.is_ok());

            let t4 = test_connect_to(addr, cloned_exec.clone()).await;
            assert!(t4.is_err());
        });
    }
    async fn test_connect_to(
        addr: SocketAddr,
        exec: Executor,
    ) -> Result<Task<()>, Box<dyn std::error::Error + Send + Sync>> {
        let stream = Async::<TcpStream>::connect(addr).await?;
        let mut cfg = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(InsecureCertAcceptor))
            .with_no_client_auth();
        cfg.alpn_protocols = vec!["h2".as_bytes().to_vec()];
        let conn = futures_rustls::TlsConnector::from(Arc::new(cfg));
        let conn = conn
            .connect("localhost".try_into().unwrap(), stream)
            .await?;
        let conn = Box::new(NativeStream::NewTlsStream(conn.into()));
        let host = format!("http://{}", addr);

        // bit of an hack here using Incoming as a type
        let h2_client = hyper::client::conn::http2::Builder::new(exec.clone())
            .handshake(conn)
            .await;
        assert!(h2_client.is_ok());
        let (mut send_request, conn) = h2_client.unwrap();
        let cloned_exec = exec.clone();
        let task = exec.spawn(async move {
            let _h2_state = cloned_exec.spawn(async move {
                let _ = conn.await;
            });
            loop {
                let req = ResourceNamesRequest::default();
                let body = encode_request(req);
                assert!(body.is_ok());
                let req = hyper::Request::builder()
                    .method(Method::POST)
                    .uri(host.clone() + "/viam.robot.v1.RobotService/ResourceNames")
                    .header(CONTENT_TYPE, "application/grpc")
                    .header(TE, "trailers")
                    .body(Full::new(body.unwrap()).boxed());
                assert!(req.is_ok());
                let req = req.unwrap();
                send_request.ready().await.unwrap();
                let resp = send_request.send_request(req).await;
                assert!(resp.is_ok());
                let (_, body) = resp.unwrap().into_parts();
                let body = body.collect().await.unwrap();
                assert!(body.trailers().is_some());
                assert_eq!(
                    body.trailers()
                        .as_ref()
                        .unwrap()
                        .get("grpc-status")
                        .unwrap()
                        .to_str()
                        .unwrap(),
                    "0"
                );
                let _ = Timer::after(Duration::from_millis(500));
            }
        });

        Ok(task)
    }

    #[test_log::test]
    /// Test that in  absence of credentials the robot starts in provisioning mode
    /// we confirm this by looking  for relevant mDNS records
    /// Once provisioning is done mDNS records should be deleted
    /// and when viam server connects to the fake app we can confirm the secrets
    /// are the one we set
    fn test_provisioning() {
        let _unused = global_network_test_lock();
        let ram_storage = RAMStorage::new();
        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };

        let mut viam_server = ViamServerBuilder2::new(ram_storage);
        let mdns = NativeMdns::new("rust-test-provisioning".to_owned(), network.get_ip());
        assert!(mdns.is_ok());
        let mdns = mdns.unwrap();
        let mut provisioning_info = ProvisioningInfo::default();
        provisioning_info.set_manufacturer("viam".to_owned());
        provisioning_info.set_model("provisioning-test".to_owned());
        viam_server
            .with_provisioning_info(provisioning_info)
            .with_app_uri("http://localhost:56563".try_into().unwrap());

        let exec = Executor::new();

        let mut viam_server = viam_server.build(
            NativeH2Connector::default(),
            exec.clone(),
            mdns,
            Box::new(network),
        );
        let cloned_exec = exec.clone();

        let mut app = AppServerInsecure::default();
        app.auth_fn = Some(Rc::new(Box::new(|req: &AuthenticateRequest| {
            assert!(req.credentials.is_some());
            assert_eq!(
                req.credentials.as_ref().unwrap().payload,
                "a-secret-test".to_owned()
            );
            assert_eq!(req.entity, "an-id-test".to_owned());
            true
        })));

        let _fake_server_task =
            exec.spawn(async move { run_fake_app_server(cloned_exec, app).await });

        let cloned_exec = exec.clone();
        exec.block_on(async move {
            let _task = cloned_exec.spawn(async move {
                viam_server.run().await;
            });
            let record = look_for_an_mdns_record(
                "_rpc._tcp.local.",
                "provisioning",
                "provisioning-test-viam",
            )
            .or(async {
                let _ = Timer::after(Duration::from_secs(1)).await;
                Err("timeout".into())
            })
            .await;

            assert!(record.is_ok());
            let record = record.unwrap();

            let addr = record.get_addresses_v4().into_iter().take(1).next();
            assert!(addr.is_some());
            let addr = addr.unwrap();
            let port = record.get_port();
            let addr = SocketAddr::new(std::net::IpAddr::V4(*addr), port);

            let ret = do_provisioning_step(cloned_exec.clone(), addr)
                .or(async {
                    let _ = Timer::after(Duration::from_secs(1)).await;
                    Err("timeout".into())
                })
                .await;
            assert!(ret.is_ok());
            Timer::after(Duration::from_secs(1)).await;

            let record = look_for_an_mdns_record(
                "_rpc._tcp.local.",
                "provisioning",
                "provisioning-test-viam",
            )
            .or(async {
                let _ = Timer::after(Duration::from_secs(1)).await;
                Err("timeout".into())
            })
            .await;

            assert!(record.is_err());
        });
    }
    async fn look_for_an_mdns_record(
        _service: &str,
        prop: &str,
        name: &str,
    ) -> Result<ServiceInfo, Box<dyn std::error::Error + Send + Sync>> {
        let mdns_querying = mdns_sd::ServiceDaemon::new();
        assert!(mdns_querying.is_ok());
        let mdns_querying = mdns_querying.unwrap();
        let service = "_rpc._tcp.local.";

        let receiver = mdns_querying.browse(service);
        assert!(receiver.is_ok());
        let receiver = receiver.unwrap();
        loop {
            let record = receiver.recv_async().await;

            if let ServiceEvent::ServiceResolved(srv) = record? {
                if srv.get_property(prop).is_some() && srv.get_hostname().contains(name) {
                    return Ok(srv);
                }
            }
        }
    }
    async fn do_provisioning_step(
        exec: Executor,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stream = async_io::Async::<TcpStream>::connect(addr).await;
        assert!(stream.is_ok());

        let host = format!("http://{}", addr);

        let stream = NativeStream::LocalPlain(stream.unwrap());
        let client = hyper::client::conn::http2::Builder::new(exec.clone())
            .handshake(stream)
            .await;

        assert!(client.is_ok());
        let (mut send_request, conn) = client.unwrap();
        let _sender = exec.spawn(async move {
            let _ = conn.await;
        });

        let mut req = SetSmartMachineCredentialsRequest::default();
        req.cloud = Some(CloudConfig {
            id: "an-id-test".to_owned(),
            secret: "a-secret-test".to_owned(),
            app_address: "".to_owned(),
        });

        let body = encode_request(req);
        assert!(body.is_ok());
        let req = hyper::Request::builder()
            .method(Method::POST)
            .uri(
                host.clone()
                    + "/viam.provisioning.v1.ProvisioningService/SetSmartMachineCredentials",
            )
            .header(CONTENT_TYPE, "application/grpc")
            .header(TE, "trailers")
            .body(Full::new(body.unwrap()).boxed());
        assert!(req.is_ok());
        let req = req.unwrap();
        assert!(send_request.ready().await.is_ok());

        let resp = send_request.send_request(req).await;
        assert!(resp.is_ok());
        let (_, body) = resp.unwrap().into_parts();
        let body = body.collect().await.unwrap();
        assert!(body.trailers().is_some());
        assert_eq!(
            body.trailers()
                .as_ref()
                .unwrap()
                .get("grpc-status")
                .unwrap()
                .to_str()
                .unwrap(),
            "0"
        );
        Ok(())
    }

    #[ignore]
    #[test_log::test]
    fn test_viam_builder() {
        let ram_storage = RAMStorage::new();
        let creds = CloudConfig {
            id: "".to_string(),
            secret: "".to_string(),
            app_address: "".to_owned(),
        };

        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };

        ram_storage.store_robot_credentials(creds).unwrap();

        let mut a = ViamServerBuilder2::new(ram_storage);
        let mdns = NativeMdns::new("".to_owned(), Ipv4Addr::new(0, 0, 0, 0)).unwrap();

        let cc = NativeH2Connector::default();
        a.with_http2_server(cc, 12346);
        a.with_app_uri("http://localhost:56563".try_into().unwrap());
        a.with_app_client_task(Box::new(RestartMonitor::new(|| {})));
        a.with_app_client_task(Box::new(LogUploadTask {}));

        let cert = Rc::new(Box::new(WebRtcCertificate::new()) as Box<dyn Certificate>);
        let dtls = Box::new(NativeDtls::new(cert.clone()));
        let exec = Executor::new();
        let conf = WebRtcConfiguration2::new(cert, dtls);
        a.with_webrtc_configuration(conf);

        let cc = NativeH2Connector::default();
        let mut b = a.build(cc, exec.clone(), mdns, Box::new(network));
        let cloned_exec = exec.clone();
        let _t = exec.spawn(async move {
            run_fake_app_server(cloned_exec, AppServerInsecure::default()).await
        });
        exec.block_on(async {
            Timer::after(Duration::from_millis(200)).await;
        });
        b.run_forever();
    }

    async fn run_fake_app_server(exec: Executor, app: AppServerInsecure) {
        let listener = Async::new(TcpListener::bind("0.0.0.0:56563").unwrap()).unwrap();
        loop {
            let (incoming, _peer) = listener.accept().await.unwrap();
            let stream = NativeStream::LocalPlain(incoming);
            let conn = http2::Builder::new(exec.clone()).serve_connection(stream, app.clone());
            let ret = conn.await;
            if ret.is_err() {
                break;
            }
        }
    }
    fn make_sample_config() -> RobotConfig {
        RobotConfig {
            cloud: Some(app::v1::CloudConfig::default()),
            ..Default::default()
        }
    }
}
