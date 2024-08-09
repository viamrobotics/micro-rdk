use std::{
    fmt::Debug,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(feature = "native")]
use std::net::SocketAddr;

use async_io::Timer;

use super::{
    app_client::{AppClient, AppClientBuilder},
    config_monitor::ConfigMonitor,
    conn::{
        network::Network,
        server::{TlsClientConnector, ViamServerBuilder, WebRtcConfiguration},
    },
    credentials_storage::{RobotConfigurationStorage, RobotCredentials, WifiCredentialStorage},
    exec::Executor,
    grpc::ServerError,
    grpc_client::GrpcClient,
    log::config_log_entry,
    provisioning::server::{serve_provisioning_async, ProvisioningInfo},
    registry::ComponentRegistry,
    restart_monitor::RestartMonitor,
    robot::LocalRobot,
};

use crate::{
    common::{app_client::AppClientError, grpc_client::GrpcClientError},
    proto::app::v1::{CloudConfig, ConfigResponse, RobotConfig},
};

#[cfg(feature = "native")]
use crate::native::{
    certificate::WebRtcCertificate,
    conn::mdns::NativeMdns,
    dtls::NativeDtls,
    tcp::NativeListener,
    tls::{NativeTls, NativeTlsServerConfig},
};

#[cfg(feature = "esp32")]
use crate::{
    common::conn::mdns::NoMdns,
    esp32::{
        certificate::GeneratedWebRtcCertificateBuilder,
        conn::mdns::Esp32Mdns,
        dtls::Esp32DtlsBuilder,
        esp_idf_svc::sys::{settimeofday, timeval},
        tls::Esp32TLS,
    },
};

pub enum RobotRepresentation {
    WithRobot(LocalRobot),
    WithRegistry(Box<ComponentRegistry>),
}

pub async fn validate_robot_credentials(
    exec: Executor,
    robot_creds: &RobotCredentials,
    client_connector: &mut impl TlsClientConnector,
) -> Result<AppClient, Box<dyn std::error::Error>> {
    let client = GrpcClient::new(
        client_connector.connect().await?,
        exec.clone(),
        "https://app.viam.com:443",
    )
    .await?;
    let builder = AppClientBuilder::new(Box::new(client), robot_creds.clone());

    builder.build().await.map_err(|e| e.into())
}

pub async fn serve_web_inner<S>(
    storage: S,
    repr: RobotRepresentation,
    exec: Executor,
    max_webrtc_connection: usize,
    network: impl Network,
    client_connector: impl TlsClientConnector,
    app_client: AppClient,
) where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    // TODO(NPM) this is a workaround so that async-io thread has started before we
    // instantiate the Async<TCPStream> for the connection to app.viam.com
    // otherwise there is a chance a race happens and will listen to events before full
    // initialization is done
    let _ = Timer::after(std::time::Duration::from_millis(60)).await;

    let robot_credentials = app_client.robot_credentials();

    let (cfg_response, cfg_received_datetime) = app_client
        .get_app_config(Some(network.get_ip()))
        .await
        .unwrap();

    let rpc_host = cfg_response
        .config
        .clone()
        .unwrap_or(RobotConfig {
            ..Default::default()
        })
        .cloud
        .clone()
        .unwrap_or(CloudConfig {
            ..Default::default()
        })
        .fqdn
        .clone();

    #[cfg(feature = "esp32")]
    if let Some(current_dt) = cfg_received_datetime.as_ref() {
        let tz = chrono_tz::Tz::UTC;
        std::env::set_var("TZ", tz.name());
        let tv_sec = current_dt.timestamp() as i32;
        let tv_usec = current_dt.timestamp_subsec_micros() as i32;
        let current_timeval = timeval { tv_sec, tv_usec };
        let res = unsafe { settimeofday(&current_timeval as *const timeval, std::ptr::null()) };
        if res != 0 {
            log::error!(
                "could not set time of day for timezone {:?} and timestamp {:?}",
                tz.name(),
                current_dt
            );
        }
    }

    let robot = match repr {
        RobotRepresentation::WithRobot(robot) => Arc::new(Mutex::new(robot)),
        RobotRepresentation::WithRegistry(registry) => {
            log::info!("building robot from config");
            let (r, err) = match LocalRobot::from_cloud_config(
                exec.clone(),
                robot_credentials.robot_id.clone(),
                &cfg_response,
                registry,
                cfg_received_datetime,
            ) {
                Ok(robot) => (robot, None),
                Err(err) => {
                    log::error!("could not build robot from config due to {:?}, defaulting to empty robot until a valid config is accessible", err);
                    (LocalRobot::new(), Some(err))
                }
            };
            if let Some(datetime) = cfg_received_datetime {
                let logs = vec![config_log_entry(datetime, err)];
                let _ = app_client.push_logs(logs).await;
            }
            Arc::new(Mutex::new(r))
        }
    };

    #[cfg(feature = "native")]
    let webrtc_certificate = WebRtcCertificate::new();
    #[cfg(feature = "esp32")]
    let webrtc_certificate = GeneratedWebRtcCertificateBuilder::default()
        .build()
        .unwrap();

    let webrtc_certificate = Rc::new(webrtc_certificate);

    #[cfg(feature = "native")]
    let dtls = NativeDtls::new(webrtc_certificate.clone());
    #[cfg(feature = "esp32")]
    let dtls = Esp32DtlsBuilder::new(webrtc_certificate.clone());

    let webrtc = Box::new(WebRtcConfiguration::new(
        webrtc_certificate,
        dtls,
        exec.clone(),
    ));

    #[cfg(feature = "native")]
    let mdns = NativeMdns::new("".to_owned(), network.get_ip()).unwrap();
    #[cfg(feature = "esp32")]
    let mdns = NoMdns {};

    #[cfg(feature = "native")]
    let restart = || std::process::exit(0);
    #[cfg(feature = "esp32")]
    let restart = || unsafe { crate::esp32::esp_idf_svc::sys::esp_restart() };

    let server_builder = ViamServerBuilder::new(
        mdns,
        exec.clone(),
        client_connector,
        robot_credentials,
        max_webrtc_connection,
        network,
        rpc_host,
    )
    .with_webrtc(webrtc)
    .with_periodic_app_client_task(Box::new(RestartMonitor::new(restart)))
    .with_periodic_app_client_task(Box::new(ConfigMonitor::new(
        *(cfg_response.clone()),
        storage.clone(),
        restart,
    )));

    #[cfg(feature = "native")]
    let server_builder = {
        server_builder.with_http2(
            {
                let certs = app_client.get_certificates().await.unwrap();
                let tls_server_config = NativeTlsServerConfig::new(
                    certs.tls_certificate.as_bytes().to_vec(),
                    certs.tls_private_key.as_bytes().to_vec(),
                );
                let address: SocketAddr = "0.0.0.0:12346".parse().unwrap();
                let tls = Box::new(NativeTls::new_server(tls_server_config));
                NativeListener::new(address.into(), Some(tls)).unwrap()
            },
            12346,
        )
    };

    let mut server = server_builder
        .with_app_client(app_client)
        .build(&cfg_response)
        .unwrap();

    // Attempt to cache the config for the machine we are about to `serve`.
    if let Err(e) = storage.store_robot_configuration(cfg_response.config.unwrap()) {
        log::warn!("Failed to store robot configuration: {}", e);
    }

    server.serve(robot).await;
}

pub async fn serve_async_with_external_network<S>(
    exec: Executor,
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
    #[cfg(feature = "native")]
    let mut client_connector = NativeTls::new_client();
    #[cfg(feature = "esp32")]
    let mut client_connector = Esp32TLS::new_client();

    let mut last_error: Option<Box<dyn std::error::Error>> = None;

    let app_client = 'app_connection: loop {
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
                        break 'app_connection app_client;
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

        #[cfg(feature = "native")]
        let mut mdns = NativeMdns::new("".to_owned(), network.get_ip()).unwrap();
        #[cfg(feature = "esp32")]
        let mut mdns = Esp32Mdns::new("".to_owned())?;

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
