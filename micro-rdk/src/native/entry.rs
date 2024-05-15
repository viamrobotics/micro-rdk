#![allow(dead_code)]

use crate::{
    common::{
        app_client::{AppClientBuilder, AppClientConfig},
        conn::{
            network::{ExternallyManagedNetwork, Network},
            server::{ViamServerBuilder, WebRtcConfiguration},
        },
        entry::RobotRepresentation,
        grpc_client::GrpcClient,
        log::config_log_entry,
        restart_monitor::RestartMonitor,
        robot::LocalRobot,
    },
    native::{exec::NativeExecutor, tcp::NativeStream, tls::NativeTls},
};
use std::{
    net::SocketAddr,
    rc::Rc,
    sync::{Arc, Mutex},
};

#[cfg(feature = "provisioning")]
use crate::common::{
    conn::mdns::Mdns,
    grpc::GrpcError,
    provisioning::{
        server::{ProvisioningInfo, ProvisioningServiceBuilder, ProvisoningServer},
        storage::RobotCredentialStorage,
    },
};
#[cfg(feature = "provisioning")]
use async_io::Async;
#[cfg(feature = "provisioning")]
use std::{
    fmt::Debug,
    net::{Ipv4Addr, TcpListener},
};

use super::{
    certificate::WebRtcCertificate, conn::mdns::NativeMdns, dtls::NativeDtls, tcp::NativeListener,
    tls::NativeTlsServerConfig,
};

#[cfg(feature = "data")]
use crate::common::{data_manager::DataManager, data_store::StaticMemoryDataStore};

pub async fn serve_web_inner(
    app_config: AppClientConfig,
    tls_server_config: NativeTlsServerConfig,
    repr: RobotRepresentation,
    exec: NativeExecutor,
    network: ExternallyManagedNetwork,
) {
    let client_connector = NativeTls::new_client();
    let mdns = NativeMdns::new("".to_owned(), network.get_ip()).unwrap();

    let (cfg_response, robot) = {
        let cloned_exec = exec.clone();
        let conn = client_connector.open_ssl_context(None).await.unwrap();
        let conn = NativeStream::TLSStream(Box::new(conn));
        let grpc_client = GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443")
            .await
            .unwrap();
        let builder = AppClientBuilder::new(Box::new(grpc_client), app_config.clone());
        log::info!("build client start");
        let client = builder.build().await.unwrap();

        let (cfg_response, cfg_received_datetime) =
            client.get_config(network.get_ip()).await.unwrap();

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

    #[cfg(feature = "data")]
    // TODO: Spawn data task here. May have to move the initialization below to the task itself
    // TODO: Support implementers of the DataStore trait other than StaticMemoryDataStore in a way that is configurable
    {
        let _data_manager_svc = DataManager::<StaticMemoryDataStore>::from_robot_and_config(
            &cfg_response,
            &app_config,
            robot.clone(),
        );
    }

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

    let mut srv =
        ViamServerBuilder::new(mdns, cloned_exec, client_connector, app_config, 3, network)
            .with_http2(tls_listener, 12346)
            .with_webrtc(webrtc)
            .with_periodic_app_client_task(Box::new(RestartMonitor::new(|| std::process::exit(0))))
            .build(&cfg_response)
            .unwrap();

    srv.serve(robot).await;
}

#[cfg(feature = "provisioning")]
async fn serve_provisioning_async<S>(
    ip: Ipv4Addr,
    exec: NativeExecutor,
    info: ProvisioningInfo,
    storage: S,
    last_error: Option<String>,
) -> Result<(AppClientConfig, NativeTlsServerConfig), Box<dyn std::error::Error>>
where
    S: RobotCredentialStorage + Clone + 'static,
    S::Error: Debug,
    GrpcError: From<S::Error>,
{
    let hostname = format!(
        "provisioning-{}-{}",
        info.get_model(),
        info.get_manufacturer()
    );
    let mut mdns = NativeMdns::new(hostname, ip)?;

    let srv = ProvisioningServiceBuilder::new().with_provisioning_info(info);

    let srv = if let Some(error) = last_error {
        srv.with_last_error(error)
    } else {
        srv
    };
    let srv = srv.build(storage.clone());
    let listen = TcpListener::bind(ip.to_string() + ":0")?;
    let listen: Async<TcpListener> = listen.try_into()?;

    let port = listen.get_ref().local_addr()?.port();

    log::info!("provisioning server run on {}:{}", ip, port);

    mdns.add_service(
        "provisioning",
        "_rpc",
        "_tcp",
        port,
        &[("provisioning", "1")],
    )?;
    loop {
        let incoming = listen.accept().await;
        let (stream, _) = incoming?;
        log::info!("will attempt to provision");

        // The provisioning server is exposed over unencrypted HTTP2
        let stream = NativeStream::LocalPlain(stream);

        let ret = ProvisoningServer::new(srv.clone(), exec.clone(), stream).await;
        if ret.is_ok() && storage.has_stored_credentials() {
            let creds = storage.get_robot_credentials().unwrap();
            let app_config = AppClientConfig::new(
                creds.robot_secret().to_owned(),
                creds.robot_id().to_owned(),
                "".to_owned(),
            );

            let conn = NativeStream::TLSStream(Box::new(
                NativeTls::new_client().open_ssl_context(None).await?,
            ));
            let client = GrpcClient::new(conn, exec.clone(), "https://app.viam.com:443").await?;
            let builder = AppClientBuilder::new(Box::new(client), app_config.clone());

            let mut client = builder.build().await?;
            let certs = client.get_certificates().await?;

            return Ok((
                app_config,
                NativeTlsServerConfig::new(
                    certs.tls_certificate.into(),
                    certs.tls_private_key.into(),
                ),
            ));
        }
    }
}

#[cfg(feature = "provisioning")]
pub fn serve_with_provisioning<S>(
    storage: S,
    info: ProvisioningInfo,
    repr: RobotRepresentation,
    network: ExternallyManagedNetwork,
) where
    S: RobotCredentialStorage + Clone + 'static,
    S::Error: Debug,
    GrpcError: From<S::Error>,
{
    let exec = NativeExecutor::new();
    let cloned_exec = exec.clone();
    let mut last_error = None;
    let (app_config, tls_server_config) = loop {
        match cloned_exec.block_on(serve_provisioning_async(
            network.get_ip(),
            exec.clone(),
            info.clone(),
            storage.clone(),
            last_error.clone(),
        )) {
            Ok(c) => break c,
            Err(e) => {
                log::error!("provisioning step failed with {:?}", e);
                let _ = last_error.insert(format!("{:?}", e));
            }
        }
    };
    serve_web(app_config, tls_server_config, repr, network)
}

pub fn serve_web(
    app_config: AppClientConfig,
    tls_server_config: NativeTlsServerConfig,
    repr: RobotRepresentation,
    network: ExternallyManagedNetwork,
) {
    let exec = NativeExecutor::new();
    let cloned_exec = exec.clone();

    cloned_exec.block_on(Box::pin(serve_web_inner(
        app_config,
        tls_server_config,
        repr,
        exec,
        network,
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

        let config = AppClientConfig::new("".to_string(), "".to_string(), "".to_owned());

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

        let p = recv_half.next().await.unwrap().unwrap().message;

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
            .map(|m| m.unwrap().message)
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
            .map(|m| m.unwrap().message)
            .collect::<String>()
            .await;
        assert_eq!("123456", p);
    }
}
