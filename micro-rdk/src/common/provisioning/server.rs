#![allow(dead_code)]
use std::{
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

use bytes::{BufMut, Bytes, BytesMut};
use futures_lite::Future;
use http_body_util::BodyExt;
use hyper::{
    body::Incoming, header::CONTENT_TYPE, http, rt, server::conn::http2, service::Service, Request,
    Response,
};
use prost::Message;

use crate::{
    common::grpc::{GrpcBody, GrpcError, GrpcResponse},
    proto::provisioning::{
        self,
        v1::{
            GetSmartMachineStatusResponse, SetSmartMachineCredentialsRequest,
            SetSmartMachineCredentialsResponse,
        },
    },
};

use super::storage::RobotCredentialStorage;

#[derive(Default)]
struct ProvisioningServiceBuilder {
    last_connection_attempt: Option<NetworkInfo>,
    provisioning_info: Option<ProvisioningInfo>,
    reason: ProvisioningReason,
}

impl ProvisioningServiceBuilder {
    fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    fn with_provisioning_info(mut self, info: ProvisioningInfo) -> Self {
        let _ = self.provisioning_info.insert(info);
        self
    }
    fn with_reason(mut self, reason: ProvisioningReason) -> Self {
        self.reason = reason;
        self
    }
    fn with_network_info(mut self, info: NetworkInfo) -> Self {
        let _ = self.last_connection_attempt.insert(info);
        self
    }
    fn build<S: RobotCredentialStorage + Clone>(self, storage: S) -> ProvisioningService<S> {
        ProvisioningService {
            provisioning_info: Rc::new(self.provisioning_info),
            last_connection_attempt: Rc::new(self.last_connection_attempt),
            reason: Rc::new(self.reason),
            storage,
            credential_ready: Rc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(PartialEq, Default)]
enum ProvisioningReason {
    #[default]
    Unprovisioned,
    InvalidCredentials,
}

#[derive(Default)]
struct NetworkInfo(provisioning::v1::NetworkInfo);
#[derive(Default)]
struct ProvisioningInfo(provisioning::v1::ProvisioningInfo);

impl ProvisioningInfo {
    fn set_fragment_id(&mut self, frag_id: String) {
        self.0.fragment_id = frag_id;
    }
    fn set_model(&mut self, model: String) {
        self.0.model = model;
    }
    fn set_manufacturer(&mut self, manufacturer: String) {
        self.0.manufacturer = manufacturer;
    }
}

#[derive(Clone)]
struct ProvisioningService<S> {
    provisioning_info: Rc<Option<ProvisioningInfo>>,
    last_connection_attempt: Rc<Option<NetworkInfo>>,
    reason: Rc<ProvisioningReason>,
    storage: S,
    credential_ready: Rc<AtomicBool>,
}

impl<S> ProvisioningService<S>
where
    S: RobotCredentialStorage + Clone,
    GrpcError: From<S::Error>,
{
    async fn process_request_inner(&self, req: Request<Incoming>) -> Result<Bytes, GrpcError> {
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
            _ => Err(GrpcError::RpcUnimplemented),
        }
    }
    fn get_smart_machine_status(&self) -> Result<Bytes, GrpcError> {
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
        resp.has_smart_machine_credentials = self.storage.has_stored_credentials();
        let len = resp.encoded_len();
        let mut buffer = BytesMut::with_capacity(5 + len);
        buffer.put_u8(0);
        buffer.put_u32(len.try_into().unwrap());
        resp.encode(&mut buffer)
            .map_err(|_| GrpcError::RpcInternal)?;
        debug_assert_eq!(buffer.len(), 5 + len);
        debug_assert_eq!(buffer.capacity(), 5 + len);
        Ok(buffer.freeze())
    }

    fn set_smart_machine_credentials(&self, body: Bytes) -> Result<Bytes, GrpcError> {
        let creds =
            SetSmartMachineCredentialsRequest::decode(body).map_err(|_| GrpcError::RpcInternal)?;
        self.storage.store_robot_credentials(creds.cloud.unwrap())?;
        let resp = SetSmartMachineCredentialsResponse::default();

        let len = resp.encoded_len();
        let mut buffer = BytesMut::with_capacity(5 + len);
        buffer.put_u8(0);
        buffer.put_u32(len.try_into().unwrap());
        resp.encode(&mut buffer)
            .map_err(|_| GrpcError::RpcInternal)?;
        debug_assert_eq!(buffer.len(), 5 + len);
        debug_assert_eq!(buffer.capacity(), 5 + len);
        self.credential_ready.store(true, Ordering::Relaxed);
        Ok(buffer.freeze())
    }

    async fn process_request(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<GrpcBody>, http::Error> {
        let mut resp = GrpcBody::new();
        match self.process_request_inner(req).await {
            Ok(bytes) => resp.put_data(bytes),
            Err(e) => resp.set_status(e.to_status("".to_string()).code, None),
        };

        Response::builder()
            .status(200)
            .header(CONTENT_TYPE, "application/grpc")
            .body(resp)
    }

    fn get_credential_ready(&self) -> Rc<AtomicBool> {
        self.credential_ready.clone()
    }
    fn reset_credential_ready(&self) {
        self.credential_ready.store(false, Ordering::Relaxed);
    }
}

impl<S> Service<Request<Incoming>> for ProvisioningService<S>
where
    S: RobotCredentialStorage + Clone + 'static,
    GrpcError: From<S::Error>,
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
struct ProvisoningServer<I, S, E>
where
    S: RobotCredentialStorage + Clone + 'static,
    GrpcError: From<S::Error>,
{
    _exec: PhantomData<E>,
    _stream: PhantomData<I>,
    _storage: PhantomData<S>,
    #[pin]
    connection: http2::Connection<I, ProvisioningService<S>, E>,
    credential_ready: Rc<AtomicBool>,
}

impl<I, S, E> Future for ProvisoningServer<I, S, E>
where
    S: RobotCredentialStorage + Clone + 'static,
    I: rt::Read + rt::Write + std::marker::Unpin + 'static,
    GrpcError: From<S::Error>,
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

        if this.credential_ready.load(Ordering::Relaxed) {
            this.connection.as_mut().graceful_shutdown();
        }

        this.connection.poll(cx)
    }
}

impl<I, S, E> ProvisoningServer<I, S, E>
where
    S: RobotCredentialStorage + Clone + 'static,
    I: rt::Read + rt::Write + std::marker::Unpin + 'static,
    GrpcError: From<S::Error>,
    E: rt::bounds::Http2ServerConnExec<
        <ProvisioningService<S> as Service<Request<Incoming>>>::Future,
        GrpcBody,
    >,
{
    fn new(service: ProvisioningService<S>, executor: E, stream: I) -> Self {
        let credential_ready = service.get_credential_ready();
        service.reset_credential_ready();
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
    use prost::Message;

    use crate::{
        common::{
            app_client::encode_request,
            provisioning::{
                server::{ProvisioningInfo, ProvisioningServiceBuilder, ProvisoningServer},
                storage::{RAMStorage, RobotCredentialStorage},
            },
        },
        native::{exec::NativeExecutor, tcp::NativeStream},
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
}
