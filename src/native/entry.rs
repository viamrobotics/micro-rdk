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
    use crate::common::board::FakeBoard;
    use crate::common::conn::server::{
        HttpListener, ViamServer, ViamServerBuilder, WebRtcConfiguration,
    };
    use crate::common::grpc::GrpcServer;
    use crate::common::grpc_client::GrpcClient;
    use crate::common::robot::{LocalRobot, ResourceMap, ResourceType};
    use crate::common::webrtc::exec::WebRtcExecutor;
    use crate::common::webrtc::grpc::{WebRtcGrpcBody, WebRtcGrpcServer};
    use crate::native::certificate::WebRtcCertificate;
    use crate::native::conn::mdns::NativeMdns;
    use crate::native::dtls::{Dtls, NativeDtls};
    use crate::native::exec::NativeExecutor;
    use crate::native::tcp::{NativeListener, NativeStream};
    use crate::native::tls::{NativeTls, NativeTlsServerConfig};

    use crate::proto::common::v1::ResourceName;
    use crate::proto::rpc::examples::echo::v1::{EchoBiDiRequest, EchoBiDiResponse};

    use futures_lite::future::block_on;
    use futures_lite::{FutureExt, StreamExt};
    use local_ip_address::local_ip;

    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};

    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

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

        let grpc_client = grpc_client.unwrap();

        let config =
            AppClientConfig::new("".to_string(), "".to_string(), Ipv4Addr::new(0, 0, 0, 0));

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

        let (mut sender_half, mut recv_half) = grpc_client
            .send_request_bidi::<EchoBiDiRequest, EchoBiDiResponse>(
                r,
                Some(EchoBiDiRequest {
                    message: "1".to_string(),
                }),
            )?;

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

    #[test_log::test]
    #[ignore]
    fn test_new_server_flow() {
        let our_ip = match local_ip().unwrap() {
            IpAddr::V4(v4) => v4,
            _ => panic!("didn't get an ipv4"),
        };
        let cfg = AppClientConfig::new("".to_string(), "".to_string(), our_ip);

        let executor = NativeExecutor::new();
        let tls = NativeTls::new_client();
        let mdns = NativeMdns::new("".to_owned(), our_ip).unwrap();

        let cfg2 = NativeTlsServerConfig::default();
        let s_tls = Box::new(NativeTls::new_server(cfg2));
        let address: SocketAddr = "0.0.0.0:12348".parse().unwrap();
        let address2: SocketAddr = "0.0.0.0:12347".parse().unwrap();

        let robot = {
            let board = Arc::new(Mutex::new(FakeBoard::new(vec![])));
            let mut res: ResourceMap = HashMap::with_capacity(1);
            res.insert(
                ResourceName {
                    namespace: "rdk".to_string(),
                    r#type: "component".to_string(),
                    subtype: "board".to_string(),
                    name: "b".to_string(),
                },
                ResourceType::Board(board),
            );
            Arc::new(Mutex::new(LocalRobot::new(res)))
        };

        // block_on(async move {
        //     let cert = Rc::new(WebRtcCertificate::new().unwrap());
        //     let listener = NativeListener::new(address.into(), Some(s_tls)).unwrap();
        //     let dtls = DtlsBuilderA::new(cert.clone());
        //     // let listener2 = NativeListener::new(address2.into(), None).unwrap();
        //     //let inc = listener.incomming();
        //     //let inc2 = listener2.incomming();
        //     //let inc3 = futures_lite::stream::pending::<&i32>();
        //     let executor = NativeExecutor::new();
        //     let cloned = executor.clone();
        //     let r = WebRtcConnector::new(cert.clone(), dtls, tls, cloned, cfg);
        //     //let mut srv = ServerListener::new(inc, inc3, executor);
        //     let l3 = Http2Listener3::new(listener);
        //     let l1 = Http2SrvText::new(l3, r, executor.clone());
        //     futures_lite::future::block_on(executor.run(l1.server_forever()));

        //srv.serve_forever(robot).await;
        // let srv = ServerBuilder::new(
        //     mdns,
        //     inc,
        //     tls,
        //     executor.clone(),
        //     12345,
        //     "https://app.viam.com:443",
        //     cfg,
        // );
        // let mut p = srv.build().unwrap();
        // p.serve_forever::<NativeStream>(robot).await.unwrap();
        // });
    }
    // async fn run_inner_test()
    #[test_log::test]
    #[ignore]
    fn test_webrtc_signaling() -> anyhow::Result<()> {
        let our_ip = match local_ip()? {
            IpAddr::V4(v4) => v4,
            _ => panic!("didn't get an ipv4"),
        };
        let cfg = AppClientConfig::new("".to_string(), "".to_string(), our_ip);

        let robot = {
            let board = Arc::new(Mutex::new(FakeBoard::new(vec![])));
            let mut res: ResourceMap = HashMap::with_capacity(1);
            res.insert(
                ResourceName {
                    namespace: "rdk".to_string(),
                    r#type: "component".to_string(),
                    subtype: "board".to_string(),
                    name: "b".to_string(),
                },
                ResourceType::Board(board),
            );
            Arc::new(Mutex::new(LocalRobot::new(res)))
        };

        let executor = NativeExecutor::new();

        let mut webrtc = {
            let executor = executor.clone();
            let tls = Box::new(NativeTls::new_client());
            let conn = tls.open_ssl_context(None)?;
            let conn = NativeStream::TLSStream(Box::new(conn));
            let grpc_client = GrpcClient::new(conn, executor.clone(), "https://app.viam.com:443")?;
            let mut app_client = AppClientBuilder::new(grpc_client, cfg).build()?;

            let cert = Rc::new(WebRtcCertificate::new()?);

            let dtls = Dtls::new(cert.clone())?;

            let webrtc = app_client
                .connect_webrtc(cert, executor.clone(), dtls)
                .unwrap();

            drop(app_client);

            webrtc
        };
        let channel = block_on(executor.run(async { webrtc.open_data_channel().await })).unwrap();

        let mut webrtc_grpc =
            WebRtcGrpcServer::new(channel, GrpcServer::new(robot, WebRtcGrpcBody::default()));

        loop {
            block_on(executor.run(async { webrtc_grpc.next_request().await.unwrap() }));
        }
    }
}
