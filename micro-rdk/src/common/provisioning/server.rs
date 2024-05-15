#![allow(dead_code)]
use std::{marker::PhantomData, net::Ipv4Addr, pin::Pin, rc::Rc};

use crate::{
    common::{
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
use bytes::{BufMut, Bytes, BytesMut};
use futures_lite::Future;
use http_body_util::BodyExt;
use hyper::{
    body::Incoming, header::CONTENT_TYPE, http, rt, server::conn::http2, service::Service, Request,
    Response,
};
use prost::Message;

use super::storage::{RobotCredentialStorage, WifiCredentialStorage};

#[derive(Default)]
pub(crate) struct ProvisioningServiceBuilder<Wifi = ()> {
    last_connection_attempt: Option<NetworkInfo>,
    provisioning_info: Option<ProvisioningInfo>,
    reason: ProvisioningReason,
    last_error: Option<String>,
    wifi_manager: Option<Wifi>,
}

impl ProvisioningServiceBuilder {
    pub(crate) fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl<Wifi: WifiManager> ProvisioningServiceBuilder<Wifi> {
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
    pub(crate) fn with_wifi_manager<W: WifiManager>(
        self,
        wifi_manager: W,
    ) -> ProvisioningServiceBuilder<W> {
        ProvisioningServiceBuilder {
            last_connection_attempt: self.last_connection_attempt,
            provisioning_info: self.provisioning_info,
            reason: self.reason,
            last_error: self.last_error,
            wifi_manager: Some(wifi_manager),
        }
    }
    pub(crate) fn build<S: RobotCredentialStorage + Clone>(
        self,
        storage: S,
    ) -> ProvisioningService<S, Wifi> {
        ProvisioningService {
            provisioning_info: Rc::new(self.provisioning_info),
            last_connection_attempt: Rc::new(self.last_connection_attempt),
            reason: Rc::new(self.reason),
            storage,
            credential_ready: AtomicSync::default(),
            last_error: self.last_error,
            wifi_manager: Rc::new(self.wifi_manager),
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
pub(crate) struct NetworkInfo(pub(crate) provisioning::v1::NetworkInfo);

#[derive(Default, Clone)]
pub struct ProvisioningInfo(provisioning::v1::ProvisioningInfo);

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

pub(crate) struct ProvisioningService<S, Wifi> {
    provisioning_info: Rc<Option<ProvisioningInfo>>,
    last_connection_attempt: Rc<Option<NetworkInfo>>,
    reason: Rc<ProvisioningReason>,
    storage: S,
    credential_ready: AtomicSync,
    last_error: Option<String>,
    wifi_manager: Rc<Option<Wifi>>,
}

impl<S: Clone, Wifi> Clone for ProvisioningService<S, Wifi> {
    fn clone(&self) -> Self {
        Self {
            provisioning_info: self.provisioning_info.clone(),
            last_connection_attempt: self.last_connection_attempt.clone(),
            reason: self.reason.clone(),
            storage: self.storage.clone(),
            credential_ready: self.credential_ready.clone(),
            last_error: self.last_error.clone(),
            wifi_manager: self.wifi_manager.clone(),
        }
    }
}

impl<S, Wifi> ProvisioningService<S, Wifi>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    Wifi: WifiManager,
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
            let creds = SetNetworkCredentialsRequest::decode(body)
                .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?;
            wifi_manager
                .try_connect(&creds.ssid, &creds.psk)
                .await
                .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?;

            let resp = SetNetworkCredentialsResponse::default();
            let len = resp.encoded_len();
            let mut buffer = BytesMut::with_capacity(5 + len);
            buffer.put_u8(0);
            buffer.put_u32(len.try_into().unwrap());
            resp.encode(&mut buffer)
                .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?;
            debug_assert_eq!(buffer.len(), 5 + len);
            debug_assert_eq!(buffer.capacity(), 5 + len);
            if self.storage.has_stored_credentials() {
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

        resp.has_smart_machine_credentials = self.storage.has_stored_credentials();
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
        self.storage.store_robot_credentials(creds.cloud.unwrap())?;
        let resp = SetSmartMachineCredentialsResponse::default();

        let len = resp.encoded_len();
        let mut buffer = BytesMut::with_capacity(5 + len);
        buffer.put_u8(0);
        buffer.put_u32(len.try_into().unwrap());
        resp.encode(&mut buffer)
            .map_err(|e| ServerError::new(GrpcError::RpcInternal, Some(e.into())))?;
        debug_assert_eq!(buffer.len(), 5 + len);
        debug_assert_eq!(buffer.capacity(), 5 + len);
        if self.wifi_manager.is_some() && self.storage.has_wifi_credentials() {
            self.credential_ready.done();
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

impl<S, Wifi> Service<Request<Incoming>> for ProvisioningService<S, Wifi>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    Wifi: WifiManager + 'static,
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
pub(crate) struct ProvisoningServer<I, S, E, Wifi>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    Wifi: WifiManager + 'static,
{
    _exec: PhantomData<E>,
    _stream: PhantomData<I>,
    _storage: PhantomData<S>,
    #[pin]
    connection: http2::Connection<I, ProvisioningService<S, Wifi>, E>,
    credential_ready: AtomicSync,
}

impl<I, S, E, Wifi> Future for ProvisoningServer<I, S, E, Wifi>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    I: rt::Read + rt::Write + std::marker::Unpin + 'static,
    E: rt::bounds::Http2ServerConnExec<
        <ProvisioningService<S, Wifi> as Service<Request<Incoming>>>::Future,
        GrpcBody,
    >,
    Wifi: WifiManager + 'static,
{
    type Output = Result<(), hyper::Error>;
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();

        this.connection.poll(cx)
    }
}

impl<I, S, E, Wifi> ProvisoningServer<I, S, E, Wifi>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    I: rt::Read + rt::Write + std::marker::Unpin + 'static,
    E: rt::bounds::Http2ServerConnExec<
        <ProvisioningService<S, Wifi> as Service<Request<Incoming>>>::Future,
        GrpcBody,
    >,
    Wifi: WifiManager + 'static,
{
    pub(crate) fn new(service: ProvisioningService<S, Wifi>, executor: E, stream: I) -> Self {
        let credential_ready = service.get_credential_ready();
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

pub(crate) trait WifiManager {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn scan_networks(&self) -> Result<Vec<NetworkInfo>, Self::Error>;
    async fn try_connect(&self, ssid: &str, password: &str) -> Result<(), Self::Error>;
    fn get_ap_ip(&self) -> Ipv4Addr;
}

impl WifiManager for () {
    type Error = ServerError;
    async fn scan_networks(&self) -> Result<Vec<NetworkInfo>, Self::Error> {
        Err(ServerError::new(GrpcError::RpcUnimplemented, None))
    }
    async fn try_connect(&self, _: &str, _: &str) -> Result<(), Self::Error> {
        Err(ServerError::new(GrpcError::RpcUnimplemented, None))
    }
    fn get_ap_ip(&self) -> Ipv4Addr {
        Ipv4Addr::UNSPECIFIED
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
        time::Duration,
    };

    use async_io::{Async, Timer};

    use http_body_util::BodyExt;
    use http_body_util::Full;
    use hyper::{
        header::{CONTENT_TYPE, TE},
        Method,
    };
    use mdns_sd::ServiceEvent;
    use prost::Message;
    use rand::{distributions::Alphanumeric, Rng};

    use crate::{
        common::{
            app_client::encode_request,
            conn::mdns::Mdns,
            provisioning::{
                server::{ProvisioningInfo, ProvisioningServiceBuilder, ProvisoningServer},
                storage::{RAMStorage, RobotCredentialStorage},
            },
        },
        native::{conn::mdns::NativeMdns, exec::NativeExecutor, tcp::NativeStream},
        proto::provisioning::v1::{
            CloudConfig, GetNetworkListRequest, GetSmartMachineStatusRequest,
            GetSmartMachineStatusResponse, SetSmartMachineCredentialsRequest,
        },
    };

    use super::ProvisioningService;

    async fn run_provisioning_server(ex: NativeExecutor, srv: ProvisioningService<RAMStorage>) {
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

    async fn test_provisioning_server_inner(exec: NativeExecutor, addr: SocketAddr) {
        let stream = async_io::Async::<TcpStream>::connect(addr.clone()).await;
        assert!(stream.is_ok());

        let host = format!("http://{}", addr.to_string());

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
        assert_eq!(resp.has_smart_machine_credentials, false);

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

        let mut req = SetSmartMachineCredentialsRequest::default();
        req.cloud = Some(CloudConfig {
            id: "an-id".to_owned(),
            secret: "a-secret".to_owned(),
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

        let stream = async_io::Async::<TcpStream>::connect(addr.clone()).await;
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
        assert_eq!(resp.has_smart_machine_credentials, true);
    }

    #[test_log::test]
    fn test_provisioning_server() {
        let exec = NativeExecutor::default();

        let mut provisioning_info = ProvisioningInfo::default();
        provisioning_info.set_fragment_id("a-fragment-id".to_owned());
        provisioning_info.set_model("a-model".to_owned());
        provisioning_info.set_manufacturer("a-manufacturer".to_owned());

        let storage = RAMStorage::default();

        let srv = ProvisioningServiceBuilder::new()
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
        ex: NativeExecutor,
        srv: ProvisioningService<MemoryCredentialStorage>,
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

        let exec = NativeExecutor::default();

        let mut provisioning_info = ProvisioningInfo::default();
        provisioning_info.set_fragment_id("a-fragment-id".to_owned());
        provisioning_info.set_model("a-model".to_owned());
        provisioning_info.set_manufacturer("a-manufacturer".to_owned());
        let storage = MemoryCredentialStorage::default();

        let srv = ProvisioningServiceBuilder::new()
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
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if info.get_properties().get("provisioning").is_some() {
                            let addr =
                                (*info.get_addresses().iter().take(1).next().unwrap()).clone();
                            let port = info.get_port();
                            return Some(SocketAddr::new(addr, port));
                        }
                    }
                    _ => {}
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
