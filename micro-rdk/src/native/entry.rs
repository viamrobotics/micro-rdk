#![allow(dead_code)]

use crate::{
    common::{
        app_client::{AppClient, AppClientError},
        config_monitor::ConfigMonitor,
        conn::{
            network::Network,
            server::{TlsClientConnector, ViamServerBuilder, WebRtcConfiguration},
        },
        entry::{validate_robot_credentials, RobotRepresentation},
        grpc_client::GrpcClientError,
        log::config_log_entry,
        restart_monitor::RestartMonitor,
        robot::LocalRobot,
    },
    native::{exec::NativeExecutor, tls::NativeTls},
};
use std::{
    net::SocketAddr,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(feature = "provisioning")]
use crate::{
    common::{
        grpc::ServerError,
        provisioning::{
            server::ProvisioningInfo,
            storage::{RobotConfigurationStorage, WifiCredentialStorage},
        },
    },
    proto::app::v1::ConfigResponse,
};

#[cfg(feature = "provisioning")]
use std::fmt::Debug;

use super::{
    certificate::WebRtcCertificate, conn::mdns::NativeMdns, dtls::NativeDtls, tcp::NativeListener,
    tls::NativeTlsServerConfig,
};

pub async fn serve_web_inner<S>(
    storage: S,
    repr: RobotRepresentation,
    exec: NativeExecutor,
    _max_webrtc_connection: usize,
    network: impl Network,
    client_connector: impl TlsClientConnector,
    mut app_client: AppClient,
) where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    let mdns = NativeMdns::new("".to_owned(), network.get_ip()).unwrap();

    let certs = app_client.get_certificates().await.unwrap();

    let tls_server_config = NativeTlsServerConfig::new(
        certs.tls_certificate.as_bytes().to_vec(),
        certs.tls_private_key.as_bytes().to_vec(),
    );

    let (app_config, cfg_response, cfg_received_datetime) =
        app_client.get_config(Some(network.get_ip())).await.unwrap();

    let robot = match repr {
        RobotRepresentation::WithRobot(robot) => Arc::new(Mutex::new(robot)),
        RobotRepresentation::WithRegistry(registry) => {
            log::info!("building robot from config");
            let r = match LocalRobot::from_cloud_config(
                exec.clone(),
                app_config.get_robot_id(),
                &cfg_response,
                registry,
                cfg_received_datetime,
            ) {
                Ok(robot) => {
                    if let Some(datetime) = cfg_received_datetime {
                        let logs = vec![config_log_entry(datetime, None)];
                        app_client
                            .push_logs(logs)
                            .await
                            .expect("could not push logs to app");
                    }
                    robot
                }
                Err(err) => {
                    if let Some(datetime) = cfg_received_datetime {
                        let logs = vec![config_log_entry(datetime, Some(err))];
                        app_client
                            .push_logs(logs)
                            .await
                            .expect("could not push logs to app");
                    }
                    // TODO(RSDK-8127) shouldn't panic here, when we support offline mode and
                    // reloading configuration this should be removed
                    panic!("couldn't build robot");
                }
            };
            Arc::new(Mutex::new(r))
        }
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

    let mut srv =
        ViamServerBuilder::new(mdns, cloned_exec, client_connector, app_config, 3, network)
            .with_http2(tls_listener, 12346)
            .with_webrtc(webrtc)
            .with_app_client(app_client)
            .with_periodic_app_client_task(Box::new(RestartMonitor::new(|| std::process::exit(0))))
            .with_periodic_app_client_task(Box::new(ConfigMonitor::new(
                || std::process::exit(0),
                *(cfg_response.clone()),
            )))
            .build(&cfg_response)
            .unwrap();

    // Attempt to cache the config for the machine we are about to `serve`.
    if let Err(e) = storage.store_robot_configuration(cfg_response.config.unwrap()) {
        log::warn!("Failed to store robot configuration: {}", e);
    }

    srv.serve(robot).await;
}

#[cfg(feature = "provisioning")]
async fn serve_async_with_external_network<S>(
    exec: NativeExecutor,
    info: Option<ProvisioningInfo>,
    storage: S,
    mut repr: RobotRepresentation,
    network: impl Network,
    max_webrtc_connection: usize,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    use async_io::Timer;

    use crate::common::provisioning::server::serve_provisioning_async;

    let mut client_connector = NativeTls::new_client();
    let info = info.unwrap_or_default();
    let mut last_error: Option<Box<dyn std::error::Error>> = None;

    let app_client = 'provisioned: loop {
        if storage.has_robot_credentials() {
            log::info!("Found cached robot credentials; attempting to serve");

            if storage.has_robot_configuration() {
                if let RobotRepresentation::WithRegistry(ref registry) = repr {
                    log::info!("Found cached robot configuration; speculatively building robot from config");
                    match LocalRobot::from_cloud_config(
                        exec.clone(),
                        storage.get_robot_credentials().unwrap().robot_id,
                        &ConfigResponse {
                            config: Some(storage.get_robot_configuration().unwrap()),
                        },
                        registry.clone(),
                        None,
                    ) {
                        Ok(robot) => {
                            repr = RobotRepresentation::WithRobot(robot);
                        }
                        Err(e) => {
                            log::warn!("Failed building robot from cached robot configuration: {}; dropping and ignoring cached config", e);
                            let _ = storage.reset_robot_configuration();
                        }
                    };
                }
            }

            let mut duration = None;
            loop {
                if let Some(duration) = duration {
                    Timer::after(duration).await;
                } else {
                    // TODO: Maybe some back-off up to a limit
                    let _ = duration.insert(Duration::from_secs(3));
                }

                log::info!("Attempting to validate stored robot credentials");
                match validate_robot_credentials(
                    exec.clone(),
                    &storage.get_robot_credentials().unwrap(),
                    &mut client_connector,
                )
                .await
                {
                    Ok(app_client) => {
                        log::info!("Robot credentials validated OK");
                        break 'provisioned app_client;
                    }
                    Err(e) => {
                        if let Some(app_client_error) = e.downcast_ref::<AppClientError>() {
                            if matches!(app_client_error, AppClientError::AppGrpcClientError(GrpcClientError::GrpcError{ code, .. }) if *code == 7 || *code == 16)
                            {
                                // The validate call failed with an explicit rejection (PERMISSION_DENIED/UNAUTHENTICATED)
                                // of the credentials. Reset the cached credentials and any robot configuration, and
                                // move on to provisioning.
                                log::warn!("Robot credential validation failed permanently with error {:?}; clearing cached state and initiating provisioning", e);

                                if let Err(e) = storage.reset_robot_credentials() {
                                    log::error!("Couldn't erase robot credentials {:?}", e);
                                }

                                if let Err(e) = storage.reset_robot_configuration() {
                                    log::error!("couldn't erase robot configuration {:?}", e);
                                }

                                // Record the last error so that we can serve it once we reach provisioning.
                                let _ = last_error.insert(e);
                                break;
                            }
                        }

                        // For all other errors, we assume we could not communicate with app due
                        // to network issues, and just restart the inner loop until we are able
                        // to communicate with app.
                        log::info!("Unable to validate robot credentials {:?}; will retry", e);
                    }
                }
            }
        }

        log::warn!("Entering provisioning...");
        let mut mdns = NativeMdns::new("".to_owned(), network.get_ip()).unwrap();
        if let Err(e) = serve_provisioning_async::<_, (), _>(
            exec.clone(),
            info.clone(),
            storage.clone(),
            last_error.take(),
            None,
            &mut mdns,
        )
        .await
        {
            let _ = last_error.insert(e);
        }
    };
    serve_web_inner(
        storage,
        repr,
        exec,
        max_webrtc_connection,
        network,
        client_connector,
        app_client,
    )
    .await;
    Ok(())
}

pub fn serve_web_with_external_network<S>(
    info: Option<ProvisioningInfo>,
    repr: RobotRepresentation,
    max_webrtc_connection: usize,
    storage: S,
    network: impl Network,
) where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    let exec = NativeExecutor::new();
    let cloned_exec = exec.clone();

    let _ = cloned_exec.block_on(Box::pin(serve_async_with_external_network(
        exec,
        info,
        storage,
        repr,
        network,
        max_webrtc_connection,
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

    use std::net::TcpStream;

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
        let grpc_client = grpc_client.unwrap();

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
