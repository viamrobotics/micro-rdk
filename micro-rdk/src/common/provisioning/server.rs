#![allow(dead_code)]
use std::{pin::Pin, rc::Rc, sync::Mutex};

use bytes::{BufMut, Bytes, BytesMut};
use futures_lite::Future;
use http_body_util::BodyExt;
use hyper::{body::Incoming, header::CONTENT_TYPE, http, service::Service, Request, Response};
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

use super::storage::Storage;

struct ProvisioningServiceBuilder {
    last_connection_attempt: Option<NetworkInfo>,
    provisioning_info: Option<ProvisioningInfo>,
    reason: ProvisioningReason,
}

impl ProvisioningServiceBuilder {
    fn new() -> Self {
        Self {
            reason: ProvisioningReason::Erased,
            last_connection_attempt: Default::default(),
            provisioning_info: Default::default(),
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
    fn build<S: Storage + Clone>(self, storage: S) -> ProvisioningService<S> {
        ProvisioningService {
            provisioning_info: Rc::new(self.provisioning_info),
            last_connection_attempt: Rc::new(self.last_connection_attempt),
            reason: Rc::new(self.reason),
            storage: Rc::new(Mutex::new(storage)),
        }
    }
}

#[derive(PartialEq)]
enum ProvisioningReason {
    Erased,
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
    storage: Rc<Mutex<S>>,
}

impl<S> ProvisioningService<S>
where
    S: Storage + Clone,
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
        resp.has_smart_machine_credentials = self.storage.lock().unwrap().has_stored_credentials();
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
        self.storage
            .lock()
            .unwrap()
            .store_robot_credentials(creds.cloud.unwrap())?;
        let resp = SetSmartMachineCredentialsResponse::default();

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

    fn get_robot_credential(&self) -> Result<(String, String), S::Error> {
        self.storage.lock().unwrap().get_robot_credentials()
    }
}

impl<S> Service<Request<Incoming>> for ProvisioningService<S>
where
    S: Storage + Clone + 'static,
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
        server, Method,
    };
    use prost::Message;

    use crate::{
        common::{
            app_client::encode_request,
            provisioning::{
                server::{ProvisioningInfo, ProvisioningServiceBuilder},
                storage::MemoryCredentialStorage,
            },
        },
        native::{exec::NativeExecutor, tcp::NativeStream},
        proto::provisioning::v1::{
            CloudConfig, GetNetworkListRequest, GetSmartMachineStatusRequest,
            GetSmartMachineStatusResponse, SetSmartMachineCredentialsRequest,
        },
    };

    use super::ProvisioningService;

    async fn run_provisioning_server(
        ex: NativeExecutor,
        srv: ProvisioningService<MemoryCredentialStorage>,
    ) {
        let listen = TcpListener::bind("127.0.0.1:56432");
        assert!(listen.is_ok());
        let listen: Async<TcpListener> = listen.unwrap().try_into().unwrap();
        loop {
            let incoming = listen.accept().await;
            assert!(incoming.is_ok());
            let (stream, _) = incoming.unwrap();

            let stream = NativeStream::LocalPlain(stream);

            let r = server::conn::http2::Builder::new(ex.clone())
                .serve_connection(stream, srv.clone())
                .await;
            assert!(r.is_ok());
        }
    }

    async fn test_provisioning_server_inner(exec: NativeExecutor) {
        Timer::after(Duration::from_millis(50)).await; // let server spin up

        let addr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 56432);

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

        let req = GetSmartMachineStatusRequest::default();
        let body = encode_request(req);
        assert!(body.is_ok());

        let req = hyper::Request::builder()
	    .method(Method::POST)
	    .uri("http://127.0.0.1:56432/viam.provisioning.v1.ProvisioningService/GetSmartMachineStatus")
	    .header(CONTENT_TYPE, "application/grpc")
	    .header(TE,"trailers")
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
            .uri("http://127.0.0.1:56432/viam.provisioning.v1.ProvisioningService/GetNetworkList")
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
            .uri("http://127.0.0.1:56432/viam.provisioning.v1.ProvisioningService/SetSmartMachineCredentials")
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
	    .uri("http://127.0.0.1:56432/viam.provisioning.v1.ProvisioningService/GetSmartMachineStatus")
	    .header(CONTENT_TYPE, "application/grpc")
	    .header(TE,"trailers")
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
        assert_eq!(resp.has_smart_machine_credentials, true);
    }

    #[test_log::test]
    fn test_provisioning_server() {
        let exec = NativeExecutor::default();

        let mut provisioning_info = ProvisioningInfo::default();
        provisioning_info.set_fragment_id("a-fragment-id".to_owned());
        provisioning_info.set_model("a-model".to_owned());
        provisioning_info.set_manufacturer("a-manufacturer".to_owned());

        let srv = ProvisioningServiceBuilder::new()
            .with_provisioning_info(provisioning_info)
            .build(MemoryCredentialStorage::default());
        let cloned_srv = srv.clone();

        let cloned = exec.clone();
        exec.spawn(async move {
            run_provisioning_server(cloned, cloned_srv).await;
        })
        .detach();

        exec.block_on(async { test_provisioning_server_inner(exec.clone()).await });

        let cred = srv.get_robot_credential().unwrap();

        assert_eq!(&cred.0, "an-id");
        assert_eq!(&cred.1, "a-secret");
    }
}
