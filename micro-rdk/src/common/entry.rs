use std::{
    fmt::Debug,
    rc::Rc,
    sync::{Arc, Mutex},
};

#[cfg(feature = "native")]
use std::net::SocketAddr;

use async_io::Timer;

use super::{
    app_client::{AppClient, AppClientBuilder, AppClientConfig},
    config_monitor::ConfigMonitor,
    conn::{
        network::Network,
        server::{TlsClientConnector, ViamServerBuilder, WebRtcConfiguration},
    },
    credentials_storage::{RobotConfigurationStorage, RobotCredentials, WifiCredentialStorage},
    grpc::ServerError,
    grpc_client::GrpcClient,
    log::config_log_entry,
    registry::ComponentRegistry,
    restart_monitor::RestartMonitor,
    robot::LocalRobot,
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
        dtls::Esp32DtlsBuilder,
        esp_idf_svc::sys::{settimeofday, timeval},
    },
};

pub enum RobotRepresentation {
    WithRobot(LocalRobot),
    WithRegistry(Box<ComponentRegistry>),
}

#[cfg(feature = "native")]
type Executor = crate::native::exec::NativeExecutor;
#[cfg(feature = "esp32")]
type Executor = crate::esp32::exec::Esp32Executor;

pub async fn validate_robot_credentials(
    exec: Executor,
    robot_creds: &RobotCredentials,
    client_connector: &mut impl TlsClientConnector,
) -> Result<AppClient, Box<dyn std::error::Error>> {
    let app_config = AppClientConfig::new(
        robot_creds.robot_secret().to_owned(),
        robot_creds.robot_id().to_owned(),
        "".to_owned(),
    );
    let client = GrpcClient::new(
        client_connector.connect().await?,
        exec.clone(),
        "https://app.viam.com:443",
    )
    .await?;
    let builder = AppClientBuilder::new(Box::new(client), app_config.clone());

    builder.build().await.map_err(|e| e.into())
}

pub async fn serve_web_inner<S>(
    storage: S,
    repr: RobotRepresentation,
    exec: Executor,
    max_webrtc_connection: usize,
    network: impl Network,
    client_connector: impl TlsClientConnector,
    mut app_client: AppClient,
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

    let (app_config, cfg_response, cfg_received_datetime) =
        app_client.get_config(Some(network.get_ip())).await.unwrap();

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
                app_config.get_robot_id(),
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
        app_config,
        max_webrtc_connection,
        network,
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
