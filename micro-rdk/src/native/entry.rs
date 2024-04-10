#![allow(dead_code)]
use crate::{
    common::{
        app_client::{AppClientBuilder, AppClientConfig},
        conn::server::{ViamServerBuilder, WebRtcConfiguration},
        entry::RobotRepresentation,
        grpc_client::GrpcClient,
        log::config_log_entry,
        robot::LocalRobot,
    },
    native::exec::NativeExecutor,
    native::tcp::NativeStream,
    native::tls::NativeTls,
};
use std::{
    net::{Ipv4Addr, SocketAddr},
    rc::Rc,
    sync::{Arc, Mutex},
};

use super::{
    certificate::WebRtcCertificate, conn::mdns::NativeMdns, dtls::NativeDtls, tcp::NativeListener,
    tls::NativeTlsServerConfig,
};

pub async fn serve_web_inner(
    app_config: AppClientConfig,
    tls_server_config: NativeTlsServerConfig,
    repr: RobotRepresentation,
    ip: Ipv4Addr,
    exec: NativeExecutor,
) {
    let client_connector = NativeTls::new_client();
    let mdns = NativeMdns::new("".to_owned(), ip).unwrap();

    let (cfg_response, robot) = {
        let cloned_exec = exec.clone();
        let conn = client_connector.open_ssl_context(None).await.unwrap();
        let conn = NativeStream::TLSStream(Box::new(conn));
        let grpc_client = GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443")
            .await
            .unwrap();
        let builder = AppClientBuilder::new(Box::new(grpc_client), app_config.clone());
        log::info!("build client start");
        let mut client = builder.build().await.unwrap();

        let (cfg_response, cfg_received_datetime) = client.get_config().await.unwrap();

        let robot = match repr {
            RobotRepresentation::WithRobot(robot) => Arc::new(Mutex::new(robot)),
            RobotRepresentation::WithRegistry(registry) => {
                log::info!("building robot from config");
                let r = match LocalRobot::from_cloud_config(
                    &cfg_response,
                    registry,
                    cfg_received_datetime,
                ) {
                    Ok(robot) => {
                        if let Some(datetime) = cfg_received_datetime {
                            let logs = vec![config_log_entry(datetime, None)];
                            client
                                .push_logs(logs)
                                .await
                                .expect("could not push logs to app");
                        }
                        robot
                    }
                    Err(err) => {
                        if let Some(datetime) = cfg_received_datetime {
                            let logs = vec![config_log_entry(datetime, Some(err))];
                            client
                                .push_logs(logs)
                                .await
                                .expect("could not push logs to app");
                        }
                        //TODO shouldn't panic here, when we support offline mode and reloading configuration this should be removed
                        panic!("couldn't build robot");
                    }
                };
                Arc::new(Mutex::new(r))
            }
        };

        (cfg_response, robot)
    };

    let address: SocketAddr = "0.0.0.0:12346".parse().unwrap();
    let tls = Box::new(NativeTls::new_server(tls_server_config));
    let tls_listener = NativeListener::new(address.into(), Some(tls)).unwrap();

    let webrtc_certificate = Rc::new(WebRtcCertificate::new());
    let dtls = NativeDtls::new(webrtc_certificate.clone());

    let cloned_exec = exec.clone();

    let webrtc = Box::new(WebRtcConfiguration::new(
        webrtc_certificate,
        dtls,
        exec.clone(),
    ));

    let mut srv = ViamServerBuilder::new(mdns, cloned_exec, client_connector, app_config, 3)
        .with_http2(tls_listener, 12346)
        .with_webrtc(webrtc)
        .build(&cfg_response)
        .unwrap();

    srv.serve(robot).await;
}

pub fn serve_web(
    app_config: AppClientConfig,
    tls_server_config: NativeTlsServerConfig,
    repr: RobotRepresentation,
    ip: Ipv4Addr,
) {
    let exec = NativeExecutor::new();
    let cloned_exec = exec.clone();

    cloned_exec.block_on(Box::pin(serve_web_inner(
        app_config,
        tls_server_config,
        repr,
        ip,
        exec,
    )));
}

#[cfg(test)]
mod tests {
    use crate::common::app_client::{encode_request, AppClientBuilder, AppClientConfig};

    use crate::common::grpc_client::GrpcClient;

    use crate::native::exec::NativeExecutor;
    use crate::native::tcp::NativeStream;
    use crate::native::tls::NativeTls;

    use crate::proto::rpc::examples::echo::v1::{EchoBiDiRequest, EchoBiDiResponse};
    use crate::proto::rpc::v1::{AuthenticateRequest, AuthenticateResponse, Credentials};

    use async_io::Async;
    use futures_lite::future::block_on;
    use futures_lite::StreamExt;
    use http_body_util::BodyExt;
    use prost::Message;

    use std::net::{Ipv4Addr, TcpStream};

    #[test_log::test]
    #[ignore]
    fn test_app_client() {
        let exec = NativeExecutor::new();
        exec.block_on(async { test_app_client_inner().await });
    }
    async fn test_app_client_inner() {
        let tls = Box::new(NativeTls::new_client());
        let conn = tls.open_ssl_context(None);
        let conn = block_on(conn);
        assert!(conn.is_ok());

        let conn = conn.unwrap();

        let conn = NativeStream::TLSStream(Box::new(conn));

        let exec = NativeExecutor::new();

        let grpc_client = GrpcClient::new(conn, exec, "https://app.viam.com:443").await;

        assert!(grpc_client.is_ok());

        let grpc_client = Box::new(grpc_client.unwrap());

        let config = AppClientConfig::new(
            "".to_string(),
            "".to_string(),
            Ipv4Addr::new(0, 0, 0, 0),
            "".to_owned(),
        );

        let builder = AppClientBuilder::new(grpc_client, config);

        let client = builder.build().await;

        assert!(client.is_ok());

        let _ = client.unwrap();
    }

    #[test_log::test]
    #[ignore]
    fn test_client_bidi() {
        let exec = NativeExecutor::new();
        exec.block_on(async { test_client_bidi_inner().await });
    }
    async fn test_client_bidi_inner() {
        let socket = TcpStream::connect("localhost:7888").unwrap();
        let socket = Async::new(socket);
        assert!(socket.is_ok());
        let socket = socket.unwrap();
        let conn = NativeStream::LocalPlain(socket);
        let executor = NativeExecutor::new();

        let grpc_client = GrpcClient::new(conn, executor, "https://app.viam.com:443").await;
        assert!(grpc_client.is_ok());
        let mut grpc_client = grpc_client.unwrap();

        let cred = Credentials {
            r#type: "robot-secret".to_owned(),
            payload: "some-secret".to_owned(),
        };

        let req = AuthenticateRequest {
            entity: "some entity".to_owned(),
            credentials: Some(cred),
        };

        let body = encode_request(req);
        assert!(body.is_ok());
        let body = body.unwrap();
        let r = grpc_client.build_request(
            "/proto.rpc.v1.AuthService/Authenticate",
            None,
            "",
            http_body_util::Full::new(body)
                .map_err(|never| match never {})
                .boxed(),
        );

        assert!(r.is_ok());
        let r = r.unwrap();

        let r = grpc_client.send_request(r).await;
        assert!(r.is_ok());
        let mut r = r.unwrap().0;
        let r = r.split_off(5);
        let r = AuthenticateResponse::decode(r).unwrap();
        let jwt = format!("Bearer {}", r.access_token);

        let (sender, receiver) = async_channel::bounded::<bytes::Bytes>(1);
        let r = grpc_client.build_request(
            "/proto.rpc.examples.echo.v1.EchoService/EchoBiDi",
            Some(&jwt),
            "",
            BodyExt::boxed(http_body_util::StreamBody::new(
                receiver.map(|b| Ok(hyper::body::Frame::data(b))),
            )),
        );
        assert!(r.is_ok());
        let r = r.unwrap();

        let conn = grpc_client
            .send_request_bidi::<EchoBiDiRequest, EchoBiDiResponse>(r, sender)
            .await;

        assert!(conn.is_ok());

        let (mut sender_half, mut recv_half) = conn.unwrap();

        let p = recv_half.next().await.unwrap().message;

        assert_eq!("1", p);

        let recv_half_ref = recv_half.by_ref();

        sender_half
            .send_message(EchoBiDiRequest {
                message: "hello".to_string(),
            })
            .await
            .unwrap();

        let p = recv_half_ref
            .take(5)
            .map(|m| m.message)
            .collect::<String>()
            .await;

        assert_eq!("hello", p);

        sender_half
            .send_message(EchoBiDiRequest {
                message: "123456".to_string(),
            })
            .await
            .unwrap();
        let p = recv_half_ref
            .take(6)
            .map(|m| m.message)
            .collect::<String>()
            .await;
        assert_eq!("123456", p);
    }
}
