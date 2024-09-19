use async_channel::{Receiver, RecvError, Sender};
use async_io::{Async, Timer};
use either::Either;

use futures_lite::{FutureExt, StreamExt};

use futures_util::stream::FuturesUnordered;
use hyper::server::conn::http2;
use hyper::{rt, Uri};
use std::future::Future;

use std::net::{SocketAddr, TcpListener};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;
use std::{fmt::Debug, net::TcpStream};

use crate::common::app_client::{
    AppClient, AppClientBuilder, AppClientError, AppSignaling, PeriodicAppClientTask,
};

use crate::common::grpc::{GrpcBody, GrpcServer, ServerError};
use crate::common::grpc_client::GrpcClient;
use crate::common::provisioning::server::{
    serve_provisioning_async, ProvisioningInfo, WifiManager,
};
use crate::common::registry::ComponentRegistry;
use crate::common::robot::LocalRobot;
use crate::common::webrtc::api::WebRtcApi;
use crate::common::webrtc::certificate::Certificate;
use crate::common::webrtc::dtls::DtlsBuilder;
use crate::common::{
    credentials_storage::{RobotConfigurationStorage, WifiCredentialStorage},
    exec::Executor,
};

use super::mdns::Mdns;
use super::network::Network;
use super::server::{WebRTCConnection, WebRtcConfiguration2};

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
            http2_server_insecure: true,
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
            mdns,
            component_registry: self.component_registry,
            provisioning_info: self.provisioning_info,
            http2_server_insecure: self.http2_server_insecure,
            http2_server_port: self.http2_server_port,
            app_uri: self.initial_app_uri,
            wifi_manager: Rc::new(self.wifi_manager),
            app_client_tasks: self.app_client_tasks,
            rpc_host: "".to_owned(),
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
    mdns: M,
    component_registry: Box<ComponentRegistry>,
    http2_server_insecure: bool,
    http2_server_port: u16,
    app_uri: Uri,
    wifi_manager: Rc<Option<Box<dyn WifiManager>>>,
    app_client_tasks: Vec<Box<dyn PeriodicAppClientTask>>,
    rpc_host: String,
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
    async fn run(&mut self) {
        // The first step is to check whether or not credentials are populated in
        // storage. If not, we should go straight to provisioning.
        if !self.storage.has_robot_credentials() {
            self.provision().await;
        }

        // assume credentials are valid for now
        let robot_creds = self.storage.get_robot_credentials().unwrap();

        // The next step is to build the robot based on the cached config (or an empty Robot)
        // 1) We are offline therefore viam server will not start webrtc listening (AppClient wil not be constructed)
        //    However we are still able to connect locally (H2) and we should cache data is the data manager exists.
        // 2) We are online, hence there is a chance the robot we created has the wrong config. By default we are going to
        //    destroy the robot previously created. And reload everything.
        // is_connected only tells us whether or not we are on a network
        // Note default will use the same executor
        let robot = self
            .storage
            .get_robot_configuration()
            .map_or(LocalRobot::default(), |cfg| {
                LocalRobot::from_cloud_config2(
                    self.executor.clone(),
                    robot_creds.robot_id.clone(),
                    &cfg,
                    &mut self.component_registry, // why do we need a mutable ref here?
                    None,
                )
                .unwrap_or_default()
            });
        let robot = Arc::new(Mutex::new(robot));

        let app_client = self.connect_to_app().await.unwrap();

        let certs = app_client.get_certificates().await.unwrap();

        if let HTTP2Server::HTTP2Connector(s) = &mut self.http2_server {
            s.set_server_certificates(
                certs.tls_certificate.into_bytes(),
                certs.tls_private_key.into_bytes(),
            );
        }
        let ip = self.network.get_ip();

        let config = app_client.get_app_config(Some(ip)).await.unwrap();
        self.rpc_host = config.0.config.unwrap().cloud.unwrap().fqdn;

        let (tx, rx) = async_channel::bounded(1);
        let inner = RobotServer::new(
            &self.http2_server,
            &self.webrtc_configuration,
            self.executor.clone(),
            robot.clone(),
            &self.mdns,
            self.http2_server_port,
            rx,
            &self.network,
        );

        self.app_client_tasks.push(Box::new(SignalingTask {
            sender: tx,
            rpc_host: self.rpc_host.clone(),
        }));
        let mut tasks: FuturesUnordered<_> = FuturesUnordered::new();
        tasks.push(Either::Right(self.run_app_client_tasks(Some(app_client))));
        tasks.push(Either::Left(inner.run()));
        while let Some(_) = tasks.next().await {}
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
    async fn run_app_client_tasks(&self, app_client: Option<AppClient>) {
        let app_client = if let Some(app) = app_client {
            app
        } else {
            self.connect_to_app().await.unwrap()
        };
        let mut app_client_tasks: FuturesUnordered<AppClientTaskRunner> = FuturesUnordered::new();
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
            res.unwrap();
        }
    }
    // I am adding provisioning in the main flow of viamserver
    // this is however outside of the scope IMO. What could be a better way?
    // We don't want the user to have to write code to handle the provisioning
    // case.
    async fn provision(&mut self) {
        let mut last_error = None;
        while let Err(e) = serve_provisioning_async(
            self.executor.clone(),
            Some(self.provisioning_info.clone()),
            self.storage.clone(),
            last_error.take(),
            self.wifi_manager.clone(),
            &mut self.mdns,
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
    mdns: &'a M,
    http2_server_port: u16,
    webrtc_signaling: Receiver<AppSignaling>,
    network: &'a Box<dyn Network>,
}

pub(crate) enum IncomingConnection2 {
    HTTP2Connection(std::io::Result<(Async<TcpStream>, SocketAddr)>),
    WebRTCConnection(Result<AppSignaling, RecvError>),
}

impl<'a, M> RobotServer<'a, M>
where
    M: Mdns,
{
    fn new(
        http2_server: &'a HTTP2Server,
        webrtc_config: &'a WebRtcListener,
        executor: Executor,
        robot: Arc<Mutex<LocalRobot>>,
        mdns: &'a M,
        http2_server_port: u16,
        webrtc_signaling: Receiver<AppSignaling>,
        network: &'a Box<dyn Network>,
    ) -> Self {
        RobotServer {
            http2_server,
            webrtc_config,
            executor,
            robot,
            mdns,
            http2_server_port,
            webrtc_signaling,
            network,
        }
    }
    async fn run(&self) {
        loop {
            let listener = async_io::Async::new(
                TcpListener::bind(format!("0.0.0.0:{}", self.http2_server_port)).unwrap(),
            )
            .unwrap();

            let r = futures_lite::future::or(
                async { IncomingConnection2::HTTP2Connection(listener.accept().await) },
                async { IncomingConnection2::WebRTCConnection(self.webrtc_signaling.recv().await) },
            );
            match r.await {
                IncomingConnection2::HTTP2Connection(conn) => {
                    if let HTTP2Server::HTTP2Connector(h) = self.http2_server {
                        let stream = conn.unwrap();
                        let io = h.accept_connection(stream.0).unwrap().await.unwrap();
                        let _ = self.server_peer_http2(io).await;
                    }
                }
                IncomingConnection2::WebRTCConnection(conn) => {
                    let sig = conn.unwrap();
                    let ip = self.network.get_ip();
                    if let WebRtcListener::WebRtc(conf) = self.webrtc_config {
                        let mut api = WebRtcApi::new(
                            self.executor.clone(),
                            sig.0,
                            sig.1,
                            conf.cert.clone(),
                            ip,
                            conf.dtls.make().unwrap(),
                        );
                        let sdp = api.answer(0).await.unwrap();
                        let mut c = WebRTCConnection {
                            webrtc_api: api,
                            sdp: sdp.0,
                            server: None,
                            robot: self.robot.clone(),
                            prio: sdp.1,
                        };
                        let p = c.open_data_channel().await.unwrap();
                        let p = c.run().await.unwrap();
                    }
                }
            }
        }
    }
    async fn server_peer_http2(&self, io: Box<dyn HTTP2Stream>) {
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
        let t = self.executor.spawn(srv).await;
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
    invoker: &'a Box<dyn PeriodicAppClientTask>, //need to impl deref?
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
            let old = std::ptr::read(&mut self.state);
            let next = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match old {
                TaskRunnerState::Run { task: _ } => TaskRunnerState::Sleep {
                    timer: res.map_or(
                        Timer::after(self.invoker.get_default_period()),
                        Timer::after,
                    ),
                },
                TaskRunnerState::Sleep { timer: _ } => TaskRunnerState::Run {
                    task: self.invoker.invoke(&self.app_client),
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
        net::{Ipv4Addr, TcpListener},
        pin::Pin,
        rc::Rc,
        time::Duration,
    };

    use async_io::{Async, Timer};
    use bytes::{BufMut, Bytes, BytesMut};
    use http_body_util::BodyExt;
    use hyper::{body::Incoming, header::CONTENT_TYPE, server::conn::http2, service::Service};
    use prost::Message;

    use crate::{
        common::{
            conn::{
                network::ExternallyManagedNetwork, server::WebRtcConfiguration2,
                viam::ViamServerBuilder2,
            },
            credentials_storage::{RAMStorage, RobotConfigurationStorage, RobotCredentials},
            exec::Executor,
            grpc::{GrpcBody, GrpcResponse},
            log::LogUploadTask,
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
            app::v1::{
                CertificateResponse, NeedsRestartRequest, NeedsRestartResponse, RobotConfig,
            },
            provisioning::v1::CloudConfig,
            robot::v1::{LogRequest, LogResponse},
            rpc::v1::{AuthenticateRequest, AuthenticateResponse},
        },
    };

    #[derive(Clone, Default)]
    struct AppServerInsecure {
        config_fn: Option<&'static dyn Fn() -> RobotConfig>,
        log_fn: Option<&'static dyn Fn()>,
    }

    impl AppServerInsecure {
        fn authenticate(&self, body: Bytes) -> Bytes {
            let req = AuthenticateRequest::decode(body).unwrap();
            let resp = AuthenticateResponse {
                access_token: "fake".to_string(),
            };
            let len = resp.encoded_len();
            let mut buffer = BytesMut::with_capacity(5 + len);
            buffer.put_u8(0);
            buffer.put_u32(len.try_into().unwrap());
            resp.encode(&mut buffer).unwrap();
            buffer.freeze()
        }
        fn log(&self, body: Bytes) -> Bytes {
            let req = LogRequest::decode(body).unwrap();
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
            let req = NeedsRestartRequest::decode(body).unwrap();

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
        fn certificates(&self, body: Bytes) -> Bytes {
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
        async fn process_request_inner(&self, req: hyper::http::Request<Incoming>) -> Bytes {
            let (parts, body) = req.into_parts();
            let mut body = body.collect().await.unwrap().to_bytes();
            match parts.uri.path() {
                "/proto.rpc.v1.AuthService/Authenticate" => self.authenticate(body.split_off(5)),
                "/viam.app.v1.RobotService/Certificate" => self.certificates(body.split_off(5)),
                "/viam.app.v1.RobotService/Log" => self.log(body.split_off(5)),
                "/viam.app.v1.RobotService/NeedsRestart" => self.needs_restart(body.split_off(5)),
                _ => panic!("unsupported uri {:?}", parts.uri.path()),
            }
        }
        async fn process_request(
            &self,
            req: hyper::http::Request<Incoming>,
        ) -> Result<hyper::http::Response<GrpcBody>, hyper::http::Error> {
            let mut resp = GrpcBody::new();
            resp.put_data(self.process_request_inner(req).await);
            hyper::http::Response::builder()
                .status(200)
                .header(CONTENT_TYPE, "application/grpc")
                .body(resp)
        }
    }
    impl Service<hyper::http::Request<Incoming>> for AppServerInsecure {
        type Error = hyper::http::Error;
        type Response = hyper::http::Response<GrpcBody>;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
        fn call(&self, req: hyper::http::Request<Incoming>) -> Self::Future {
            let svc = self.clone();

            Box::pin(async move { svc.process_request(req).await })
        }
    }

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

        ram_storage.store_robot_credentials(creds);

        let mut a = ViamServerBuilder2::new(ram_storage);
        let mdns = NativeMdns::new("".to_owned(), Ipv4Addr::new(0, 0, 0, 0)).unwrap();

        let cc = NativeH2Connector::default();
        a.with_http2_server(cc, 12346);
        a.with_app_uri("http://localhost:56563".try_into().unwrap());
        a.with_app_client_task(Box::new(RestartMonitor::new(|| {
            log::info!("hello restart")
        })));
        a.with_app_client_task(Box::new(LogUploadTask {}));

        let cert = Rc::new(Box::new(WebRtcCertificate::new()) as Box<dyn Certificate>);
        let dtls = Box::new(NativeDtls::new(cert.clone()));
        let exec = Executor::new();
        let conf = WebRtcConfiguration2::new(cert, dtls);
        a.with_webrtc_configuration(conf);

        let cc = NativeH2Connector::default();
        let mut b = a.build(cc, exec.clone(), mdns, Box::new(network));
        let cloned_exec = exec.clone();
        let t = exec.spawn(async move { run_fake_app_server(cloned_exec).await });
        exec.block_on(async {
            Timer::after(Duration::from_millis(200)).await;
        });
        b.run_forever();
    }

    async fn run_fake_app_server(exec: Executor) {
        let svc = AppServerInsecure::default();
        let listener = Async::new(TcpListener::bind("0.0.0.0:56563").unwrap()).unwrap();
        loop {
            let (incoming, peer) = listener.accept().await.unwrap();
            log::info!("peer is {:?}", peer);
            let stream = NativeStream::LocalPlain(incoming);
            let conn = http2::Builder::new(exec.clone()).serve_connection(stream, svc.clone());
            conn.await;
        }
    }
    fn make_sample_config() {}
}
