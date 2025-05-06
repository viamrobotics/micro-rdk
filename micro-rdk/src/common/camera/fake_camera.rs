use crate::common::camera::{Camera, CameraError, CameraType};
use bytes::Bytes;
use std::sync::{Arc, Mutex};

use crate::common::{config::ConfigType, registry::ComponentRegistry, registry::Dependency};

static FAKE_JPEG: &[u8] = include_bytes!("./fake_image.jpg");

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_camera("fake", &FakeCamera::from_config)
        .is_err()
    {
        log::error!("fake camera type is already registered");
    }
}

#[derive(DoCommand)]
pub struct FakeCamera {}

impl FakeCamera {
    pub fn new() -> Self {
        FakeCamera {}
    }
    pub(crate) fn from_config(
        _cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<CameraType, CameraError> {
        Ok(Arc::new(Mutex::new(FakeCamera::new())))
    }
}

impl Default for FakeCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl Camera for FakeCamera {
    fn get_image(&mut self) -> Result<Bytes, CameraError> {
        Ok(FAKE_JPEG.into())
    }
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use std::{
        convert::Infallible,
        net::{SocketAddr, TcpListener, TcpStream},
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_io::Timer;

    use super::FAKE_JPEG;
    use crate::{
        common::{
            app_client::encode_request,
            config::{DynamicComponentConfig, Model, ResourceName},
            exec::Executor,
            grpc::{GrpcBody, GrpcError, GrpcServer},
            registry::ComponentRegistry,
            robot::{LocalRobot, RobotError},
        },
        google::api::HttpBody,
        native::tcp::NativeStream,
        proto::component::camera::v1::{GetImageRequest, GetImageResponse, RenderFrameRequest},
    };

    use http_body_util::{combinators::BoxBody, BodyExt, Collected, Full};
    use hyper::{
        body::Incoming,
        client::conn::http2::SendRequest,
        header::{CONTENT_TYPE, TE},
        server::conn::http2,
        Method,
    };
    use prost::Message;

    static SUCCESS: i32 = 0;

    fn setup_robot() -> Result<LocalRobot, RobotError> {
        let mut robot = LocalRobot::default();

        let mut conf = Vec::new();

        #[cfg(feature = "camera")]
        conf.push(Some(DynamicComponentConfig {
            name: ResourceName::new_builtin("camera".to_owned(), "camera".to_owned()),
            model: Model::new_builtin("fake".to_owned()),
            attributes: None,
            data_collector_configs: vec![],
        }));
        let mut registry: Box<ComponentRegistry> = Box::default();

        robot.process_components(conf, &mut registry)?;

        Ok(robot)
    }

    async fn setup_grpc_server(exec: Executor, addr: SocketAddr) {
        let tcp_server = TcpListener::bind(addr);
        assert!(tcp_server.is_ok());
        let tcp_server = tcp_server.unwrap();
        let listener: async_io::Async<TcpListener> = tcp_server.try_into().unwrap();

        let robot = Arc::new(Mutex::new(setup_robot().unwrap()));

        loop {
            let incoming = listener.accept().await;
            assert!(incoming.is_ok());
            let incoming = incoming.unwrap();
            let stream: NativeStream = NativeStream::LocalPlain(incoming.0);
            let srv = GrpcServer::new(robot.clone(), GrpcBody::new());
            Box::new(http2::Builder::new(exec.clone()).serve_connection(stream, srv))
                .await
                .unwrap();
        }
    }

    async fn check_response(
        resp: hyper::Response<Incoming>,
        code: i32,
    ) -> Result<Collected<bytes::Bytes>, String> {
        let (parts, body) = resp.into_parts();
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
            code.to_string()
        );

        assert_eq!(parts.status, 200);
        Ok(body)
    }

    async fn build_request<M: Message + bytes::Buf + 'static>(
        host: String,
        path: String,
        message: M,
    ) -> hyper::Request<BoxBody<M, Infallible>> {
        hyper::Request::builder()
            .method(Method::POST)
            .uri(host + &path)
            .header(CONTENT_TYPE, "application/grpc")
            .header(TE, "trailers")
            .body(Full::new(message).boxed())
            .unwrap()
    }

    async fn test_get_image(
        mut send_request: SendRequest<BoxBody<bytes::Bytes, Infallible>>,
        host: &str,
    ) -> Result<(), String> {
        let get_image_path = "/viam.component.camera.v1.CameraService/GetImage";
        // valid
        let mut message = GetImageRequest::default();
        message.name = "camera".to_string();
        let message = encode_request(message).unwrap();

        assert!(send_request.ready().await.is_ok());
        let req = build_request(host.to_string(), get_image_path.to_string(), message).await;

        let resp = send_request.send_request(req).await;
        assert!(resp.is_ok());
        let body = check_response(resp.unwrap(), SUCCESS).await.unwrap();

        let resp = GetImageResponse::decode(body.to_bytes().split_off(5));
        assert!(resp.is_ok());
        let resp = resp.unwrap();
        assert_eq!(resp.mime_type, "image/jpeg");
        assert_eq!(resp.image.len(), FAKE_JPEG.len());

        // invalid
        let mut message = GetImageRequest::default();
        message.name = "non-existant-camera".to_string();
        let message = encode_request(message).unwrap();

        assert!(send_request.ready().await.is_ok());
        let req = build_request(host.to_string(), get_image_path.to_string(), message).await;

        let resp = send_request.send_request(req).await;
        assert!(resp.is_ok());
        let _body = check_response(resp.unwrap(), GrpcError::RpcUnavailable as i32)
            .await
            .unwrap();

        Ok(())
    }

    async fn test_render_frame(
        mut send_request: SendRequest<BoxBody<bytes::Bytes, Infallible>>,
        host: &str,
    ) -> Result<(), String> {
        let get_image_path = "/viam.component.camera.v1.CameraService/RenderFrame";
        // valid
        let mut message = RenderFrameRequest::default();
        message.name = "camera".to_string();
        let message = encode_request(message).unwrap();

        assert!(send_request.ready().await.is_ok());
        let req = build_request(host.to_string(), get_image_path.to_string(), message).await;
        let resp = send_request.send_request(req).await;
        assert!(resp.is_ok());

        let body = check_response(resp.unwrap(), SUCCESS).await.unwrap();
        let resp = HttpBody::decode(body.to_bytes().split_off(5));

        assert!(resp.is_ok());
        let resp = resp.unwrap();
        assert_eq!(resp.content_type, "image/jpeg");
        assert_eq!(resp.data.len(), FAKE_JPEG.len());

        // invalid
        let mut message = RenderFrameRequest::default();
        message.name = "non-existant-camera".to_string();
        let message = encode_request(message).unwrap();

        assert!(send_request.ready().await.is_ok());
        let req = build_request(host.to_string(), get_image_path.to_string(), message).await;

        let resp = send_request.send_request(req).await;
        assert!(resp.is_ok());
        let _body = check_response(resp.unwrap(), GrpcError::RpcUnavailable as i32)
            .await
            .unwrap();

        Ok(())
    }

    #[test_log::test]
    fn test_fake_camera() {
        let exec = Executor::default();

        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();

        let cloned = exec.clone();
        exec.spawn(async move {
            setup_grpc_server(cloned, addr).await;
        })
        .detach();

        let host = format!("http://{}", addr);
        exec.block_on(async {
            Timer::after(Duration::from_millis(100)).await;
        });
        let stream = exec.block_on(async { async_io::Async::<TcpStream>::connect(addr).await });
        let stream = match stream {
            Ok(s) => NativeStream::LocalPlain(s),
            Err(e) => {
                println!("{:?}", e.to_string());
                panic!();
            }
        };

        let send_request: SendRequest<BoxBody<bytes::Bytes, Infallible>> = exec.block_on(async {
            let client = hyper::client::conn::http2::Builder::new(exec.clone())
                .handshake(stream)
                .await;

            assert!(client.is_ok());
            let (send_request, conn) = client.unwrap();
            exec.spawn(async move {
                let _ = conn.await;
            })
            .detach();
            send_request
        });

        let get_image = exec.block_on(async { test_get_image(send_request.clone(), &host).await });
        assert!(get_image.is_ok());
        let render_frame =
            exec.block_on(async { test_render_frame(send_request.clone(), &host).await });
        assert!(render_frame.is_ok());
    }
}
