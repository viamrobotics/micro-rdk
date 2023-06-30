#![allow(dead_code)]
use crate::{
    common::{
        app_client::{AppClientBuilder, AppClientConfig},
        conn::server::{ViamServerBuilder, WebRtcConfiguration},
        grpc_client::GrpcClient,
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

pub fn serve_web(
    app_config: AppClientConfig,
    tls_server_config: NativeTlsServerConfig,
    robot: Option<LocalRobot>,
    ip: Ipv4Addr,
) {
    let robot = Arc::new(Mutex::new(robot.unwrap()));

    let client_connector = NativeTls::new_client();
    let exec = NativeExecutor::new();
    let mdns = NativeMdns::new("".to_owned(), ip).unwrap();

    let robot_cfg = {
        let cloned_exec = exec.clone();
        let conn = client_connector.open_ssl_context(None).unwrap();
        let conn = NativeStream::TLSStream(Box::new(conn));
        let grpc_client = GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443").unwrap();
        let builder = AppClientBuilder::new(Box::new(grpc_client), app_config.clone());

        let mut client = builder.build().unwrap();
        client.get_config().unwrap()
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
        client_connector,
        exec.clone(),
        app_config,
    ));

    let mut srv = ViamServerBuilder::new(mdns, tls_listener, webrtc, cloned_exec, 12346)
        .build(&robot_cfg)
        .unwrap();

    srv.serve_forever(robot);
}

#[cfg(test)]
mod tests {
    use crate::common::app_client::{AppClientBuilder, AppClientConfig};

    use crate::common::grpc_client::GrpcClient;

    use crate::native::exec::NativeExecutor;
    use crate::native::tcp::NativeStream;
    use crate::native::tls::NativeTls;

    use crate::proto::rpc::examples::echo::v1::{EchoBiDiRequest, EchoBiDiResponse};

    use futures_lite::future::block_on;
    use futures_lite::StreamExt;

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

        let grpc_client = GrpcClient::new(conn, exec, "https://app.viam.com:443");

        assert!(grpc_client.is_ok());

        let grpc_client = Box::new(grpc_client.unwrap());

        let config = AppClientConfig::new(
            "".to_string(),
            "".to_string(),
            Ipv4Addr::new(0, 0, 0, 0),
            "".to_owned(),
        );

        let builder = AppClientBuilder::new(grpc_client, config);

        let client = builder.build();

        assert!(client.is_ok());

        let _ = client.unwrap();
    }

    #[test_log::test]
    #[ignore]
    fn test_client_bidi() -> anyhow::Result<()> {
        let socket = TcpStream::connect("127.0.0.1:8080").unwrap();
        socket.set_nonblocking(true)?;
        let conn = NativeStream::LocalPlain(socket);
        let executor = NativeExecutor::new();
        let mut grpc_client = GrpcClient::new(conn, executor.clone(), "http://localhost")?;

        let r = grpc_client.build_request(
            "/proto.rpc.examples.echo.v1.EchoService/EchoBiDi",
            None,
            "",
        )?;

        let conn = block_on(executor.run(async {
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

        let (p, mut recv_half) = block_on(executor.run(async {
            let p = recv_half.next().await.unwrap().message;
            (p, recv_half)
        }));
        assert_eq!("1", p);

        let recv_half_ref = recv_half.by_ref();

        sender_half.send_message(EchoBiDiRequest {
            message: "hello".to_string(),
        })?;

        let p = block_on(executor.run(async {
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
        let p = block_on(executor.run(async {
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
