#![allow(dead_code)]
use futures_lite::future::block_on;

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
    exec: NativeExecutor<'_>,
) {
    let client_connector = NativeTls::new_client();
    let mdns = NativeMdns::new("".to_owned(), ip).unwrap();

    let (cfg_response, robot) = {
        let cloned_exec = exec.clone();
        let conn = client_connector.open_ssl_context(None).unwrap();
        let conn = NativeStream::TLSStream(Box::new(conn));
        let grpc_client = GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443")
            .await
            .unwrap();
        let builder = AppClientBuilder::new(Box::new(grpc_client), app_config.clone());

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
                            let logs = vec![config_log_entry(datetime, Some(&err))];
                            client
                                .push_logs(logs)
                                .await
                                .expect("could not push logs to app");
                        }
                        panic!("{}", err)
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

    let webrtc_certificate = Rc::new(WebRtcCertificate::new().unwrap());
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

    let fut = cloned_exec.run(Box::pin(serve_web_inner(
        app_config,
        tls_server_config,
        repr,
        ip,
        exec,
    )));
    futures_lite::pin!(fut);
    block_on(fut);
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

    use futures_lite::future::block_on;
    use futures_lite::StreamExt;
    use prost::Message;

    use std::net::{Ipv4Addr, TcpStream};

    #[test_log::test]
    #[ignore]
    fn test_app_client() {
        let tls = Box::new(NativeTls::new_client());
        let conn = tls.open_ssl_context(None);

        assert!(conn.is_ok());

        let conn = conn.unwrap();

        let conn = NativeStream::TLSStream(Box::new(conn));

        let exec = NativeExecutor::new();

        let cloned_exec = exec.clone();

        let grpc_client = block_on(
            cloned_exec
                .run(async { GrpcClient::new(conn, exec, "https://app.viam.com:443").await }),
        );

        assert!(grpc_client.is_ok());

        let grpc_client = Box::new(grpc_client.unwrap());

        let config = AppClientConfig::new(
            "".to_string(),
            "".to_string(),
            Ipv4Addr::new(0, 0, 0, 0),
            "".to_owned(),
        );

        let builder = AppClientBuilder::new(grpc_client, config);

        let client = block_on(cloned_exec.run(async { builder.build().await }));

        assert!(client.is_ok());

        let _ = client.unwrap();
    }

    #[test_log::test]
    #[ignore]
    fn test_client_bidi() -> anyhow::Result<()> {
        let socket = TcpStream::connect("localhost:7888").unwrap();
        socket.set_nonblocking(true)?;
        let conn = NativeStream::LocalPlain(socket);
        let executor = NativeExecutor::new();
        let exec = executor.clone();
        let mut grpc_client = block_on(
            exec.run(async { GrpcClient::new(conn, executor, "https://app.viam.com:443").await }),
        )?;

        let r = grpc_client.build_request("/proto.rpc.v1.AuthService/Authenticate", None, "")?;

        let cred = Credentials {
            r#type: "robot-secret".to_owned(),
            payload: "some-secret".to_owned(),
        };

        let req = AuthenticateRequest {
            entity: "some entity".to_owned(),
            credentials: Some(cred),
        };

        let body = encode_request(req)?;

        let mut r = block_on(exec.run(async { grpc_client.send_request(r, body).await }))?.0;
        let r = r.split_off(5);
        let r = AuthenticateResponse::decode(r).unwrap();
        let jwt = format!("Bearer {}", r.access_token);

        let r = grpc_client.build_request(
            "/proto.rpc.examples.echo.v1.EchoService/EchoBiDi",
            Some(&jwt),
            "",
        )?;

        let conn = block_on(exec.run(async {
            grpc_client
                .send_request_bidi::<EchoBiDiRequest, EchoBiDiResponse>(
                    r,
                    Some(EchoBiDiRequest {
                        message: "1".to_string(),
                    }),
                )
                .await
        }));

        assert!(conn.is_ok());

        let (mut sender_half, mut recv_half) = conn.unwrap();

        let (p, mut recv_half) = block_on(exec.run(async {
            let p = recv_half.next().await.unwrap().message;
            (p, recv_half)
        }));
        assert_eq!("1", p);

        let recv_half_ref = recv_half.by_ref();

        sender_half.send_message(EchoBiDiRequest {
            message: "hello".to_string(),
        })?;

        let p = block_on(exec.run(async {
            recv_half_ref
                .take(5)
                .map(|m| m.message)
                .collect::<String>()
                .await
        }));

        assert_eq!("hello", p);

        sender_half.send_message(EchoBiDiRequest {
            message: "123456".to_string(),
        })?;
        let p = block_on(exec.run(async {
            recv_half_ref
                .take(6)
                .map(|m| m.message)
                .collect::<String>()
                .await
        }));

        assert_eq!("123456", p);
        Ok(())
    }
}
