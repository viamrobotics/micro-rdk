#![allow(dead_code)]
use std::{
    cell::RefCell,
    fmt::Debug,
    marker::PhantomData,
    net::{Ipv4Addr, TcpListener, UdpSocket},
    pin::Pin,
    rc::Rc,
};

use crate::{
    common::{
        config::NetworkSetting,
        conn::{
            mdns::Mdns,
            network::{Network, NetworkError},
        },
        credentials_storage::{RobotConfigurationStorage, WifiCredentialStorage},
        exec::Executor,
        grpc::{GrpcBody, GrpcError, GrpcResponse, ServerError},
        webrtc::api::AtomicSync,
    },
    proto::provisioning::{
        self,
        v1::{
            GetNetworkListResponse, GetSmartMachineStatusResponse, SetNetworkCredentialsRequest,
            SetNetworkCredentialsResponse, SetSmartMachineCredentialsRequest,
            SetSmartMachineCredentialsResponse,
        },
    },
};
use async_executor::Task;
use async_io::Async;
use bytes::{BufMut, Bytes, BytesMut};
use futures_lite::Future;
use http_body_util::BodyExt;
use hyper::{
    body::Incoming, header::CONTENT_TYPE, http, rt, server::conn::http2, service::Service, Request,
    Response,
};
use prost::Message;
use thiserror::Error;

async fn dns_server(ap_ip: Ipv4Addr) {
    let socket = async_io::Async::<UdpSocket>::bind(([0, 0, 0, 0], 53)).unwrap();
    loop {
        let mut buf = [0_u8; 512];
        let len = socket.recv_from(&mut buf).await.unwrap();
        let buf = Bytes::copy_from_slice(&buf[..len.0]);
        let mut ans = dns_message_parser::Dns::decode(buf);
        if let Ok(ref mut msg) = ans {
            if let Some(q) = msg.questions.first() {
                if q.domain_name.to_string().contains("viam.setup") {
                    let rr = dns_message_parser::rr::RR::A(dns_message_parser::rr::A {
                        domain_name: q.domain_name.clone(),
                        ttl: 3600,
                        ipv4_addr: ap_ip,
                    });

                    msg.answers.push(rr);
                    msg.flags.qr = true;

                    let buf = msg.encode().unwrap();
                    socket.send_to(&buf, len.1).await.unwrap();
                } else {
                    msg.flags.qr = true;
                    msg.flags.rcode = dns_message_parser::RCode::ServFail;
                }
            }
        }
        drop(ans);
    }
}

pub(crate) struct ProvisioningServiceBuilder<Exec> {
    last_connection_attempt: Option<NetworkInfo>,
    provisioning_info: Option<ProvisioningInfo>,
    reason: ProvisioningReason,
    last_error: Option<String>,
    wifi_manager: Rc<Option<Box<dyn WifiManager>>>,
    executor: Exec,
}

impl<Exec> ProvisioningServiceBuilder<Exec> {
    pub(crate) fn new(executor: Exec) -> Self {
        Self {
            wifi_manager: Rc::new(None),
            last_connection_attempt: None,
            provisioning_info: None,
            reason: ProvisioningReason::Unprovisioned,
            last_error: None,
            executor,
        }
    }
}

impl<Exec> ProvisioningServiceBuilder<Exec> {
    pub(crate) fn with_provisioning_info(mut self, info: ProvisioningInfo) -> Self {
        let _ = self.provisioning_info.insert(info);
        self
    }
    pub(crate) fn with_reason(mut self, reason: ProvisioningReason) -> Self {
        self.reason = reason;
        self
    }
    pub(crate) fn with_network_info(mut self, info: NetworkInfo) -> Self {
        let _ = self.last_connection_attempt.insert(info);
        self
    }
    pub(crate) fn with_last_error(mut self, error: String) -> Self {
        let _ = self.last_error.insert(error);
        self
    }
    pub(crate) fn with_wifi_manager(
        self,
        wifi_manager: Rc<Option<Box<dyn WifiManager>>>,
    ) -> ProvisioningServiceBuilder<Exec> {
        ProvisioningServiceBuilder {
            last_connection_attempt: self.last_connection_attempt,
            provisioning_info: self.provisioning_info,
            reason: self.reason,
            last_error: self.last_error,
            wifi_manager,
            executor: self.executor,
        }
    }
    pub(crate) fn build<S: RobotConfigurationStorage + Clone>(
        self,
        storage: S,
    ) -> ProvisioningService<S>
    where
        Exec: ProvisioningExecutor,
    {
        // Provisioning relies on DNS query to find the IP of the server. Specifically it will
        // make a request for viam.setup. All other queries are answered failed to express the lack of
        // internet
        let dns_task = self
            .wifi_manager
            .as_ref()
            .as_ref()
            .map(|wifi_manager| self.executor.spawn(dns_server(wifi_manager.get_ap_ip())));

        ProvisioningService {
            provisioning_info: Rc::new(self.provisioning_info),
            last_connection_attempt: Rc::new(self.last_connection_attempt),
            reason: Rc::new(self.reason),
            storage,
            credential_ready: AtomicSync::default(),
            last_error: self.last_error,
            wifi_manager: self.wifi_manager,
            dns_task: Rc::new(dns_task),
        }
    }
}

#[derive(PartialEq, Default)]
pub(crate) enum ProvisioningReason {
    #[default]
    Unprovisioned,
    InvalidCredentials,
}

#[derive(Default, Debug)]
pub struct NetworkInfo(pub(crate) provisioning::v1::NetworkInfo);

#[derive(Default, Clone)]
pub struct ProvisioningInfo(crate::proto::provisioning::v1::ProvisioningInfo);

impl ProvisioningInfo {
    pub fn set_fragment_id(&mut self, frag_id: String) {
        self.0.fragment_id = frag_id;
    }
    pub fn set_model(&mut self, model: String) {
        self.0.model = model;
    }
    pub fn set_manufacturer(&mut self, manufacturer: String) {
        self.0.manufacturer = manufacturer;
    }
    pub fn get_model(&self) -> &str {
        &self.0.model
    }
    pub fn get_manufacturer(&self) -> &str {
        &self.0.manufacturer
    }
}

pub(crate) trait ProvisioningExecutor {
    fn spawn<F: Future<Output = ()> + 'static>(&self, future: F) -> Task<()>;
}

pub(crate) struct ProvisioningService<S> {
    provisioning_info: Rc<Option<ProvisioningInfo>>,
    last_connection_attempt: Rc<Option<NetworkInfo>>,
    reason: Rc<ProvisioningReason>,
    storage: S,
    credential_ready: AtomicSync,
    last_error: Option<String>,
    wifi_manager: Rc<Option<Box<dyn WifiManager>>>,
    dns_task: Rc<Option<Task<()>>>,
}

impl<S: Clone> Clone for ProvisioningService<S> {
    fn clone(&self) -> Self {
        Self {
            provisioning_info: self.provisioning_info.clone(),
            last_connection_attempt: self.last_connection_attempt.clone(),
            reason: self.reason.clone(),
            storage: self.storage.clone(),
            credential_ready: self.credential_ready.clone(),
            last_error: self.last_error.clone(),
            wifi_manager: self.wifi_manager.clone(),
            dns_task: self.dns_task.clone(),
        }
    }
}

impl<S> ProvisioningService<S>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
{
    async fn process_request_inner(&self, req: Request<Incoming>) -> Result<Bytes, ServerError> {
        let (parts, body) = req.into_parts();
        let mut body = body
            .collect()
            .await
            .map_err(|_| GrpcError::RpcFailedPrecondition)?
            .to_bytes();
        match parts.uri.path() {
            "/viam.provisioning.v1.ProvisioningService/GetSmartMachineStatus" => {
                self.get_smart_machine_status()
            }
            "/viam.provisioning.v1.ProvisioningService/SetSmartMachineCredentials" => {
                self.set_smart_machine_credentials(body.split_off(5))
            }
            "/viam.provisioning.v1.ProvisioningService/GetNetworkList" => {
                self.get_network_list().await
            }
            "/viam.provisioning.v1.ProvisioningService/SetNetworkCredentials" => {
                self.set_network_credential_request(body.split_off(5)).await
            }
            _ => Err(ServerError::new(GrpcError::RpcUnimplemented, None)),
        }
    }
    async fn set_network_credential_request(&self, body: Bytes) -> Result<Bytes, ServerError> {
        if let Some(wifi_manager) = self.wifi_manager.as_ref() {
            let network: NetworkSetting = SetNetworkCredentialsRequest::decode(body)
                .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?
                .into();

            // may not be the best place to attempt to validate passed credentials
            wifi_manager
                .try_connect(&network.ssid, &network.password)
                .await
                .map_err(|err| {
                    ServerError::new(GrpcError::RpcInvalidArgument, Some(Box::new(err)))
                })?;

            self.storage
                .store_default_network(&network.ssid, &network.password)
                .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(Box::new(e.into()))))?;

            let resp = SetNetworkCredentialsResponse::default();
            let len = resp.encoded_len();
            let mut buffer = BytesMut::with_capacity(5 + len);
            buffer.put_u8(0);
            buffer.put_u32(len.try_into().unwrap());
            resp.encode(&mut buffer)
                .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?;
            debug_assert_eq!(buffer.len(), 5 + len);
            debug_assert_eq!(buffer.capacity(), 5 + len);
            if self.storage.has_robot_credentials() {
                self.credential_ready.done();
            }
            Ok(buffer.freeze())
        } else {
            Err(ServerError::new(GrpcError::RpcUnimplemented, None))
        }
    }
    async fn get_network_list(&self) -> Result<Bytes, ServerError> {
        if let Some(wifi_manager) = self.wifi_manager.as_ref() {
            let networks = wifi_manager
                .scan_networks()
                .await
                .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?;

            let resp = GetNetworkListResponse {
                networks: networks.into_iter().map(|m| m.0).collect(),
            };
            let len = resp.encoded_len();
            let mut buffer = BytesMut::with_capacity(5 + len);
            buffer.put_u8(0);
            buffer.put_u32(len.try_into().unwrap());
            resp.encode(&mut buffer)
                .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?;
            debug_assert_eq!(buffer.len(), 5 + len);
            debug_assert_eq!(buffer.capacity(), 5 + len);
            Ok(buffer.freeze())
        } else {
            Err(ServerError::new(GrpcError::RpcUnimplemented, None))
        }
    }
    fn get_smart_machine_status(&self) -> Result<Bytes, ServerError> {
        let mut resp = GetSmartMachineStatusResponse::default();
        if let Some(info) = self.provisioning_info.as_ref() {
            resp.provisioning_info = Some(info.0.clone());
        }
        if let Some(info) = self.last_connection_attempt.as_ref() {
            resp.latest_connection_attempt = Some(info.0.clone());
        }
        if self.reason.as_ref() == &ProvisioningReason::InvalidCredentials {
            resp.errors
                .push("stored credentials are invalid".to_owned())
        }
        if let Some(error) = self.last_error.as_ref() {
            resp.errors.push(error.clone())
        }

        resp.has_smart_machine_credentials = self.storage.has_robot_credentials();
        let len = resp.encoded_len();
        let mut buffer = BytesMut::with_capacity(5 + len);
        buffer.put_u8(0);
        buffer.put_u32(len.try_into().unwrap());
        resp.encode(&mut buffer)
            .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?;

        debug_assert_eq!(buffer.len(), 5 + len);
        debug_assert_eq!(buffer.capacity(), 5 + len);
        Ok(buffer.freeze())
    }

    fn set_smart_machine_credentials(&self, body: Bytes) -> Result<Bytes, ServerError> {
        let creds =
            SetSmartMachineCredentialsRequest::decode(body).map_err(|_| GrpcError::RpcInternal)?;
        self.storage
            .store_robot_credentials(creds.cloud.as_ref().unwrap())?;
        let resp = SetSmartMachineCredentialsResponse::default();

        let len = resp.encoded_len();
        let mut buffer = BytesMut::with_capacity(5 + len);
        buffer.put_u8(0);
        buffer.put_u32(len.try_into().unwrap());
        resp.encode(&mut buffer)
            .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?;
        debug_assert_eq!(buffer.len(), 5 + len);
        debug_assert_eq!(buffer.capacity(), 5 + len);

        match self.wifi_manager.as_ref() {
            Some(_) => {
                if self.storage.has_default_network() {
                    self.credential_ready.done()
                }
            }
            None => self.credential_ready.done(),
        }

        Ok(buffer.freeze())
    }

    async fn process_request(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<GrpcBody>, http::Error> {
        let mut resp = GrpcBody::new();
        match self.process_request_inner(req).await {
            Ok(bytes) => resp.put_data(bytes),
            Err(e) => resp.set_status(e.status_code(), Some(e.to_string())),
        };

        Response::builder()
            .status(200)
            .header(CONTENT_TYPE, "application/grpc")
            .body(resp)
    }

    pub(crate) fn get_credential_ready(&self) -> AtomicSync {
        self.credential_ready.clone()
    }
    fn reset_credential_ready(&self) {
        self.credential_ready.reset()
    }
}

impl<S> Service<Request<Incoming>> for ProvisioningService<S>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
{
    type Response = Response<GrpcBody>;
    type Error = http::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let svc = self.clone();
        Box::pin(async move { svc.process_request(req).await })
    }
}
#[pin_project::pin_project]
pub(crate) struct ProvisoningServer<I, S, E>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
{
    _exec: PhantomData<E>,
    _stream: PhantomData<I>,
    _storage: PhantomData<S>,
    #[pin]
    connection: http2::Connection<I, ProvisioningService<S>, E>,
    credential_ready: AtomicSync,
}

impl<I, S, E> Future for ProvisoningServer<I, S, E>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    I: rt::Read + rt::Write + std::marker::Unpin + 'static,
    E: rt::bounds::Http2ServerConnExec<
        <ProvisioningService<S> as Service<Request<Incoming>>>::Future,
        GrpcBody,
    >,
{
    type Output = Result<(), hyper::Error>;
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = self.project();
        if this.credential_ready.get() {
            this.connection.as_mut().graceful_shutdown();
        }
        this.connection.poll(cx)
    }
}

impl<I, S, E> ProvisoningServer<I, S, E>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    I: rt::Read + rt::Write + std::marker::Unpin + 'static,
    E: rt::bounds::Http2ServerConnExec<
        <ProvisioningService<S> as Service<Request<Incoming>>>::Future,
        GrpcBody,
    >,
{
    pub(crate) fn new(service: ProvisioningService<S>, executor: E, stream: I) -> Self {
        let credential_ready = service.get_credential_ready();
        credential_ready.reset();
        let connection = http2::Builder::new(executor).serve_connection(stream, service);
        Self {
            _exec: PhantomData,
            _stream: PhantomData,
            _storage: PhantomData,
            connection,
            credential_ready,
        }
    }
}

pub struct WifiApConfiguration {
    pub(crate) ap_ip_addr: Ipv4Addr,
    pub(crate) ssid: String,
    pub(crate) password: String,
}
impl Default for WifiApConfiguration {
    fn default() -> Self {
        #[allow(unused_mut)]
        let mut mac_address = [0_u8; 8];
        #[cfg(feature = "esp32")]
        unsafe {
            esp_idf_svc::sys::esp!(esp_idf_svc::sys::esp_efuse_mac_get_default(
                mac_address.as_mut_ptr()
            ))
            .unwrap();
        };

        let ssid = format!(
            "esp32-micrordk-{:02X}{:02X}",
            mac_address[4], mac_address[5]
        );

        let password = "viamsetup".to_string();

        log::info!("Provisioning SSID: {} - Password: {}", ssid, password);

        Self {
            ssid,
            password,
            ap_ip_addr: Ipv4Addr::new(10, 42, 0, 1),
        }
    }
}
impl WifiApConfiguration {
    pub fn set_ap_ip(mut self, ip: Ipv4Addr) -> Self {
        self.ap_ip_addr = ip;
        self
    }
    pub fn set_ap_ssid(mut self, ssid: String) -> Self {
        self.ssid = ssid;
        self
    }
    pub fn set_ap_password(mut self, password: String) -> Self {
        self.password = password;
        self
    }
}

#[derive(Error, Debug)]
pub enum WifiManagerError {
    #[error("cannot assign to heapless string")]
    HeaplessStringError,
    #[cfg(feature = "esp32")]
    #[error(transparent)]
    EspError(#[from] crate::esp32::esp_idf_svc::sys::EspError),
    #[error(transparent)]
    OtherError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error(transparent)]
    NetworError(#[from] NetworkError),
}

pub trait WifiManager: Network {
    fn scan_networks(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<NetworkInfo>, WifiManagerError>> + '_>>;
    fn try_connect<'a>(
        &'a self,
        ssid: &'a str,
        password: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WifiManagerError>> + 'a>>;
    fn get_ap_ip(&self) -> Ipv4Addr;
    fn set_ap_sta_mode(
        &self,
        conifg_ap: WifiApConfiguration,
    ) -> Pin<Box<dyn Future<Output = Result<(), WifiManagerError>> + '_>>;
    fn set_sta_mode(
        &self,
        credential: NetworkSetting,
    ) -> Pin<Box<dyn Future<Output = Result<(), WifiManagerError>> + '_>>;
    fn try_connect_by_priority(
        &self,
        networks: Vec<NetworkSetting>,
    ) -> Pin<Box<dyn Future<Output = Result<(), WifiManagerError>> + '_>>;
}

pub trait AsNetwork {
    fn as_network(&self) -> &dyn Network;
}

impl<T: Network> AsNetwork for T {
    fn as_network(&self) -> &dyn Network {
        self
    }
}

#[cfg(feature = "native")]
type Stream = crate::native::tcp::NativeStream;
#[cfg(feature = "esp32")]
type Stream = crate::esp32::tcp::Esp32Stream;

pub(crate) async fn accept_connections<S>(
    listener: Async<TcpListener>,
    service: ProvisioningService<S>,
    exec: Executor,
) where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
{
    // Annoyingly VIAM app creates a new HTTP2 connection for each provisioning request
    loop {
        let incoming = listener.accept().await;

        if let Ok((stream, _)) = incoming {
            // The provisioning server is exposed over unencrypted HTTP2
            let stream = Stream::LocalPlain(stream);
            let cloned_srv = service.clone();
            let cloned_exec = exec.clone();
            exec.spawn(async {
                if let Err(e) = ProvisoningServer::new(cloned_srv, cloned_exec, stream).await {
                    log::error!("provisioning error {:?}", e);
                }
            })
            .detach();
        } else {
            break;
        }
    }
}

pub(crate) async fn serve_provisioning_async<S, M>(
    exec: Executor,
    info: Option<ProvisioningInfo>,
    storage: S,
    last_error: Option<Box<dyn std::error::Error>>,
    wifi_manager: Rc<Option<Box<dyn WifiManager>>>,
    mdns: &RefCell<M>,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    M: Mdns,
{
    let info = info.unwrap_or_default();
    let hostname = format!(
        "provisioning-{}-{}",
        info.get_model(),
        info.get_manufacturer()
    );

    let srv = ProvisioningServiceBuilder::<_>::new(exec.clone()).with_provisioning_info(info);
    let srv = srv.with_wifi_manager(wifi_manager);

    let srv = if let Some(error) = last_error {
        srv.with_last_error(error.to_string())
    } else {
        srv
    };

    let srv = srv.build(storage.clone());
    let listen = TcpListener::bind("0.0.0.0:4772")?; // VIAM app expects the server to be at 4772
    let listen: Async<TcpListener> = listen.try_into()?;
    let port = listen.get_ref().local_addr()?.port();

    log::info!(
        "provisioning server listening at {}",
        listen.get_ref().local_addr()?
    );

    {
        let mut borrowed_mdns = mdns.borrow_mut();
        borrowed_mdns.set_hostname(&hostname)?;
        borrowed_mdns.add_service(
            "provisioning",
            "_rpc",
            "_tcp",
            port,
            &[("provisioning", "")],
        )?;

        log::info!(
            "provisioning server now discoverable via mDNS at {}.local",
            hostname
        );
    }

    let credential_ready = srv.get_credential_ready();

    let cloned_exec = exec.clone();

    let provisioning_server_task = exec.spawn(accept_connections(listen, srv, cloned_exec));

    // Future will complete when either robot credentials have been transmitted when WiFi provisioning is disabled
    // or when both robot credentials and WiFi credentials have been transmitted.
    // wait for provisioning completion
    log::info!("waiting for provisioning server to obtain credentials");
    credential_ready.await;
    log::info!("provisioning server has obtained the desired credentials");

    provisioning_server_task.cancel().await;
    let mut mdns = mdns.borrow_mut();
    if let Err(e) = mdns.remove_service("provisioning", "_rpc", "_tcp") {
        log::error!("provisioning couldn't remove mdns record error {:?}", e);
    }

    log::info!("provisioning server terminating");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
        time::Duration,
    };

    use async_io::{Async, Timer};

    use crate::native::tcp::NativeStream;
    use crate::{common::exec::Executor, tests::global_network_test_lock};
    use crate::{
        common::{
            app_client::encode_request,
            conn::mdns::Mdns,
            credentials_storage::{RAMStorage, RobotConfigurationStorage},
            provisioning::server::{
                ProvisioningInfo, ProvisioningServiceBuilder, ProvisoningServer,
            },
        },
        native::conn::mdns::NativeMdns,
        proto::provisioning::v1::{
            CloudConfig, GetNetworkListRequest, GetSmartMachineStatusRequest,
            GetSmartMachineStatusResponse, SetSmartMachineCredentialsRequest,
        },
    };
    use http_body_util::BodyExt;
    use http_body_util::Full;
    use hyper::{
        header::{CONTENT_TYPE, TE},
        Method,
    };
    use mdns_sd::ServiceEvent;
    use prost::Message;
    use rand::{distributions::Alphanumeric, Rng};

    use super::ProvisioningService;

    async fn run_provisioning_server(ex: Executor, srv: ProvisioningService<RAMStorage>) {
        let listen = TcpListener::bind("127.0.0.1:56432");
        assert!(listen.is_ok());
        let listen: Async<TcpListener> = listen.unwrap().try_into().unwrap();
        loop {
            let incoming = listen.accept().await;
            assert!(incoming.is_ok());
            let (stream, _) = incoming.unwrap();

            let stream = NativeStream::LocalPlain(stream);

            let r = ProvisoningServer::new(srv.clone(), ex.clone(), stream).await;

            assert!(r.is_ok());
        }
    }

    async fn test_provisioning_server_inner(exec: Executor, addr: SocketAddr) {
        let stream = async_io::Async::<TcpStream>::connect(addr).await;
        assert!(stream.is_ok());

        let host = format!("http://{}", addr);

        let stream = NativeStream::LocalPlain(stream.unwrap());

        let client = hyper::client::conn::http2::Builder::new(exec.clone())
            .handshake(stream)
            .await;

        assert!(client.is_ok());
        let (mut send_request, conn) = client.unwrap();
        exec.spawn(async move {
            let _ = conn.await;
        })
        .detach();

        let req = GetSmartMachineStatusRequest::default();
        let body = encode_request(req);
        assert!(body.is_ok());

        let req = hyper::Request::builder()
            .method(Method::POST)
            .uri(host.clone() + "/viam.provisioning.v1.ProvisioningService/GetSmartMachineStatus")
            .header(CONTENT_TYPE, "application/grpc")
            .header(TE, "trailers")
            .body(Full::new(body.unwrap()).boxed());
        assert!(req.is_ok());
        let req = req.unwrap();

        assert!(send_request.ready().await.is_ok());

        let resp = send_request.send_request(req).await;
        assert!(resp.is_ok());

        let (parts, body) = resp.unwrap().into_parts();
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

        assert_eq!(parts.status, 200);

        let resp = GetSmartMachineStatusResponse::decode(body.to_bytes().split_off(5));
        assert!(resp.is_ok());
        let resp = resp.unwrap();
        assert_eq!(
            resp.provisioning_info.as_ref().unwrap().manufacturer,
            "a-manufacturer"
        );
        assert_eq!(resp.provisioning_info.as_ref().unwrap().model, "a-model");
        assert_eq!(
            resp.provisioning_info.as_ref().unwrap().fragment_id,
            "a-fragment-id"
        );
        assert!(!resp.has_smart_machine_credentials);

        let req = GetNetworkListRequest::default();
        let body = encode_request(req);
        assert!(body.is_ok());

        let req = hyper::Request::builder()
            .method(Method::POST)
            .uri(host.clone() + "/viam.provisioning.v1.ProvisioningService/GetNetworkList")
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
            "12"
        );

        let req = SetSmartMachineCredentialsRequest {
            cloud: Some(CloudConfig {
                id: "an-id".to_owned(),
                secret: "a-secret".to_owned(),
                app_address: "http://localhost:56563".to_owned(),
            }),
        };

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

        let req = GetSmartMachineStatusRequest::default();
        let body = encode_request(req);
        assert!(body.is_ok());

        let req = hyper::Request::builder()
            .method(Method::POST)
            .uri(host.clone() + "/viam.provisioning.v1.ProvisioningService/GetSmartMachineStatus")
            .header(CONTENT_TYPE, "application/grpc")
            .header(TE, "trailers")
            .body(Full::new(body.unwrap()).boxed());
        assert!(req.is_ok());
        let req = req.unwrap();

        let ret = send_request.ready().await;
        assert!(ret.is_err());
        assert!(ret.err().unwrap().is_closed());

        let stream = async_io::Async::<TcpStream>::connect(addr).await;
        assert!(stream.is_ok());

        let stream = NativeStream::LocalPlain(stream.unwrap());

        let client = hyper::client::conn::http2::Builder::new(exec.clone())
            .handshake(stream)
            .await;

        assert!(client.is_ok());
        let (mut send_request, conn) = client.unwrap();
        exec.spawn(async move {
            let _ = conn.await;
        })
        .detach();

        assert!(send_request.ready().await.is_ok());

        let resp = send_request.send_request(req).await;
        assert!(resp.is_ok());

        let (parts, body) = resp.unwrap().into_parts();
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

        assert_eq!(parts.status, 200);

        let resp = GetSmartMachineStatusResponse::decode(body.to_bytes().split_off(5));
        assert!(resp.is_ok());
        let resp = resp.unwrap();
        assert_eq!(
            resp.provisioning_info.as_ref().unwrap().manufacturer,
            "a-manufacturer"
        );
        assert_eq!(resp.provisioning_info.as_ref().unwrap().model, "a-model");
        assert_eq!(
            resp.provisioning_info.as_ref().unwrap().fragment_id,
            "a-fragment-id"
        );
        assert!(resp.has_smart_machine_credentials);
    }

    #[test_log::test]
    fn test_provisioning_server() {
        let _unused = global_network_test_lock();
        let exec = Executor::default();

        let mut provisioning_info = ProvisioningInfo::default();
        provisioning_info.set_fragment_id("a-fragment-id".to_owned());
        provisioning_info.set_model("a-model".to_owned());
        provisioning_info.set_manufacturer("a-manufacturer".to_owned());

        let storage = RAMStorage::default();

        let srv = ProvisioningServiceBuilder::<_>::new(exec.clone())
            .with_provisioning_info(provisioning_info)
            .build(storage.clone());

        let cloned = exec.clone();
        exec.spawn(async move {
            run_provisioning_server(cloned, srv).await;
        })
        .detach();

        let addr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 56432);
        exec.block_on(async {
            Timer::after(Duration::from_millis(100)).await;
        });
        exec.block_on(async { test_provisioning_server_inner(exec.clone(), addr).await });

        let cred = storage.get_robot_credentials().unwrap();

        assert_eq!(cred.robot_id(), "an-id");
        assert_eq!(cred.robot_secret(), "a-secret");
    }

    async fn run_provisioning_server_with_mdns(
        ex: Executor,
        srv: ProvisioningService<RAMStorage>,
        mut mdns: NativeMdns,
        ip: Ipv4Addr,
    ) {
        let listen = TcpListener::bind(ip.to_string() + ":0");
        assert!(listen.is_ok());
        let listen: Async<TcpListener> = listen.unwrap().try_into().unwrap();
        let port = listen.get_ref().local_addr().unwrap().port();

        let ret = mdns.add_service(
            "provisioning",
            "_rpc",
            "_tcp",
            port,
            &[("provisioning", "")],
        );
        assert!(ret.is_ok());

        loop {
            let incoming = listen.accept().await;
            assert!(incoming.is_ok());
            let (stream, _) = incoming.unwrap();

            let stream = NativeStream::LocalPlain(stream);

            let r = ProvisoningServer::new(srv.clone(), ex.clone(), stream).await;

            assert!(r.is_ok());
        }
    }

    #[test_log::test]
    fn test_provisioning_server_with_mdns() {
        let _unused = global_network_test_lock();
        let ip = local_ip_address::local_ip().unwrap();
        let ip = match ip {
            std::net::IpAddr::V4(v4) => v4,
            _ => panic!(),
        };

        let hostname = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from)
            .collect::<String>();

        let mdns = NativeMdns::new(hostname, ip);
        assert!(mdns.is_ok());
        let mdns = mdns.unwrap();
        let daemon = mdns.daemon();

        let exec = Executor::default();

        let mut provisioning_info = ProvisioningInfo::default();
        provisioning_info.set_fragment_id("a-fragment-id".to_owned());
        provisioning_info.set_model("a-model".to_owned());
        provisioning_info.set_manufacturer("a-manufacturer".to_owned());
        let storage = RAMStorage::default();

        let srv = ProvisioningServiceBuilder::<_>::new(exec.clone())
            .with_provisioning_info(provisioning_info)
            .build(storage.clone());

        let cloned = exec.clone();
        exec.spawn(async move {
            run_provisioning_server_with_mdns(cloned, srv, mdns, ip).await;
        })
        .detach();
        exec.block_on(async {
            Timer::after(Duration::from_millis(100)).await;
        });

        let server_addr = daemon.browse("_rpc._tcp.local.");
        assert!(server_addr.is_ok());
        let server_addr = server_addr.unwrap();

        let addr = exec.block_on(async {
            while let Ok(event) = server_addr.recv_async().await {
                if let ServiceEvent::ServiceResolved(info) = event {
                    if info.get_properties().get("provisioning").is_some() {
                        let addr = *info.get_addresses().iter().take(1).next().unwrap();
                        let port = info.get_port();
                        return Some(SocketAddr::new(addr, port));
                    }
                }
            }
            None
        });
        assert!(daemon.stop_browse("_rpc._tcp.local.").is_ok());

        assert!(addr.is_some());
        let addr = addr.unwrap();

        exec.block_on(async { test_provisioning_server_inner(exec.clone(), addr).await });

        let cred = storage.get_robot_credentials().unwrap();

        assert_eq!(cred.robot_id(), "an-id");
        assert_eq!(cred.robot_secret(), "a-secret");
    }
}
