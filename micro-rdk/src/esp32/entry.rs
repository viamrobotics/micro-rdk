#![allow(dead_code)]

use std::{
    ffi::CString,
    fmt::Debug,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::common::{
    app_client::{AppClient, AppClientError},
    config_monitor::ConfigMonitor,
    conn::{
        mdns::NoMdns,
        network::Network,
        server::{TlsClientConnector, ViamServerBuilder, WebRtcConfiguration},
    },
    entry::RobotRepresentation,
    grpc_client::GrpcClientError,
    log::config_log_entry,
    restart_monitor::RestartMonitor,
    robot::LocalRobot,
};

use super::{
    certificate::GeneratedWebRtcCertificateBuilder,
    dtls::Esp32DtlsBuilder,
    exec::Esp32Executor,
    tls::{Esp32TLS, Esp32TLSServerConfig},
};

use async_io::Timer;
use esp_idf_svc::sys::{settimeofday, timeval};

#[cfg(feature = "provisioning")]
use crate::common::provisioning::server::ProvisioningInfo;

use crate::common::{
    credentials_storage::{RobotConfigurationStorage, WifiCredentialStorage},
    entry::validate_robot_credentials,
    grpc::ServerError,
};
use crate::proto::app::v1::ConfigResponse;

pub async fn serve_web_inner<S>(
    storage: S,
    repr: RobotRepresentation,
    exec: Esp32Executor,
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

    let webrtc_certificate = GeneratedWebRtcCertificateBuilder::default()
        .build()
        .unwrap();

    let mdns = NoMdns {};

    let certs = app_client.get_certificates().await.unwrap();

    let serv_key = CString::new(certs.tls_private_key).unwrap();
    let serv_key_len = serv_key.as_bytes_with_nul().len() as u32;
    let serv_key: *const u8 = serv_key.into_raw() as *const u8;

    let tls_certs = CString::new(certs.tls_certificate)
        .unwrap()
        .into_bytes_with_nul();
    let _tls_server_config = Esp32TLSServerConfig::new(tls_certs, serv_key, serv_key_len);

    let (app_config, cfg_response, cfg_received_datetime) =
        app_client.get_config(Some(network.get_ip())).await.unwrap();

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

    let webrtc_certificate = Rc::new(webrtc_certificate);
    let dtls = Esp32DtlsBuilder::new(webrtc_certificate.clone());

    let cloned_exec = exec.clone();
    let webrtc = Box::new(WebRtcConfiguration::new(
        webrtc_certificate,
        dtls,
        exec.clone(),
    ));

    let mut srv = ViamServerBuilder::new(
        mdns,
        cloned_exec,
        client_connector,
        app_config,
        max_webrtc_connection,
        network,
    )
    .with_webrtc(webrtc)
    .with_app_client(app_client)
    .with_periodic_app_client_task(Box::new(RestartMonitor::new(|| unsafe {
        crate::esp32::esp_idf_svc::sys::esp_restart()
    })))
    .with_periodic_app_client_task(Box::new(ConfigMonitor::new(
        *(cfg_response.clone()),
        storage.clone(),
        || unsafe { crate::esp32::esp_idf_svc::sys::esp_restart() },
    )))
    .build(&cfg_response)
    .unwrap();

    // Attempt to cache the config for the machine we are about to `serve`.
    if let Err(e) = storage.store_robot_configuration(cfg_response.config.unwrap()) {
        log::warn!("Failed to store robot configuration: {}", e);
    }

    srv.serve(robot).await;
}

// Four cases:
// 1) No Robot Credentials + WiFi without external network
// 2) No Robot Credentials with external network
// 3) Robot Credentials with external network
// 4) Robot Credentials + WiFi without external network
// The function attempts to connect to the configured Wifi network if any, it then checks the robot credentials. If Wifi credentials are absent it starts provisioning mode
// If they are invalid or absent it will start the provisioning server. Once provision is done it invokes the main server.
async fn serve_async<S>(
    exec: Esp32Executor,
    #[cfg(feature = "provisioning")] info: Option<ProvisioningInfo>,
    storage: S,
    mut repr: RobotRepresentation,
    max_webrtc_connection: usize,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    #[cfg(feature = "provisioning")]
    use crate::{
        common::provisioning::server::serve_provisioning_async,
        esp32::{
            conn::mdns::Esp32Mdns, provisioning::wifi_provisioning::Esp32WifiProvisioningBuilder,
        },
    };

    use super::conn::network::Esp32WifiNetwork;

    let mut client_connector = Esp32TLS::new_client();
    #[cfg(feature = "provisioning")]
    let info = info.unwrap_or_default();
    #[cfg(feature = "provisioning")]
    let mut last_error: Option<Box<dyn std::error::Error>> = None;

    let (network, app_client) = 'app_connection: loop {
        if storage.has_robot_credentials() && storage.has_wifi_credentials() {
            log::info!("Found cached network and robot credentials; attempting to serve");

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

            log::info!("Attempting to create network with cached credentials");
            match Esp32WifiNetwork::new(storage.get_wifi_credentials().unwrap()).await {
                Ok(mut network) => {
                    let mut duration = None;
                    loop {
                        if let Some(duration) = duration {
                            Timer::after(duration).await;
                        } else {
                            // TODO: Maybe some back-off up to a limit
                            let _ = duration.insert(Duration::from_secs(3));
                        }

                        // If we have not yet connected to the network,
                        // attempt to do so now. If we cannot even determine
                        // if we are connected, consider this a permanent
                        // error and move on to provisioning.
                        match network.is_connected() {
                            Ok(true) => {}
                            Ok(false) => match network.connect().await {
                                Ok(_) => {}
                                Err(e) => {
                                    log::info!(
                                        "Couldn't connect to network '{}' due to error {:?}; will retry",
                                        storage.get_wifi_credentials().unwrap().ssid,
                                        e
                                    );
                                    continue;
                                }
                            },
                            Err(e) => {
                                #[cfg(feature = "provisioning")]
                                {
                                    log::warn!("Couldn't determine network connectivity due to {:?}; initiating provisioning", e);
                                    let _ = last_error.insert(e.into());
                                    break;
                                }
                                #[cfg(not(feature = "provisioning"))]
                                return Err(Box::new(e));
                            }
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
                                break 'app_connection (network, app_client);
                            }
                            Err(e) => {
                                if let Some(app_client_error) = e.downcast_ref::<AppClientError>() {
                                    if matches!(app_client_error, AppClientError::AppGrpcClientError(GrpcClientError::GrpcError{ code, .. }) if *code == 7 || *code == 16)
                                    {
                                        // The validate call failed with an explicit rejection (PERMISSION_DENIED/UNAUTHENTICATED)
                                        // of the credentials. Reset the cached credentials and any robot configuration, and
                                        // move on to provisioning.
                                        log::warn!("Robot credential validation failed permanently with error {:?}; initiating provisioning", e);

                                        if let Err(e) = storage.reset_robot_credentials() {
                                            log::error!("Couldn't erase robot credentials {:?}", e);
                                        }

                                        if let Err(e) = storage.reset_robot_configuration() {
                                            log::error!(
                                                "couldn't erase robot configuration {:?}",
                                                e
                                            );
                                        }

                                        #[cfg(feature = "provisioning")]
                                        {
                                            // Record the last error so that we can serve it once we reach provisioning.
                                            let _ = last_error.insert(e);
                                            break;
                                        }
                                        #[cfg(not(feature = "provisioning"))]
                                        return Err(e);
                                    }
                                }

                                // For all other errors, we assume we could not communicate with app due
                                // to network issues, and just restart the inner loop until we are able
                                // to communicate with app.
                                log::info!(
                                    "Unable to validate robot credentials {:?}; will retry",
                                    e
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    #[cfg(feature = "provisioning")]
                    {
                        log::info!(
                            "Unable to create network with cached credentials; initiating provisioning"
                        );
                        // If we can't even construct the network
                        // with the cached wifi credentials, fall
                        // back to provisioning.
                        let _ = last_error.insert(e.into());
                    }
                    #[cfg(not(feature = "provisioning"))]
                    return Err(Box::new(e));
                }
            };
        }
        #[cfg(feature = "provisioning")]
        {
            log::warn!("Entering provisioning...");

            // Start the WiFi in AP + STA mode
            let wifi_manager = Esp32WifiProvisioningBuilder::default()
                .build(storage.clone())
                .await
                .unwrap();

            let mut mdns = Esp32Mdns::new("".to_owned())?;
            if let Err(e) = serve_provisioning_async(
                exec.clone(),
                info.clone(),
                storage.clone(),
                last_error.take(),
                Some(wifi_manager),
                &mut mdns,
            )
            .await
            {
                let _ = last_error.insert(e);
            }
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

// serve_async variant where an external network is provided
#[cfg(feature = "provisioning")]
async fn serve_async_with_external_network<S>(
    exec: Esp32Executor,
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
    use crate::common::provisioning::server::serve_provisioning_async;

    use super::conn::mdns::Esp32Mdns;

    let mut client_connector = Esp32TLS::new_client();
    let info = info.unwrap_or_default();
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
                                log::warn!("Robot credential validation failed permanently with error {:?}; initiating provisioning", e);

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

pub fn serve_web<S>(
    #[cfg(feature = "provisioning")] info: Option<ProvisioningInfo>,
    repr: RobotRepresentation,
    max_webrtc_connection: usize,
    storage: S,
) where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    // set the TWDT to expire after 5 minutes
    crate::esp32::esp_idf_svc::sys::esp!(unsafe {
        crate::esp32::esp_idf_svc::sys::esp_task_wdt_init(300, true)
    })
    .unwrap();

    // Register the current task on the TWDT. The TWDT runs in the IDLE Task.
    crate::esp32::esp_idf_svc::sys::esp!(unsafe {
        crate::esp32::esp_idf_svc::sys::esp_task_wdt_add(
            crate::esp32::esp_idf_svc::sys::xTaskGetCurrentTaskHandle(),
        )
    })
    .unwrap();

    let exec = Esp32Executor::new();
    let cloned_exec = exec.clone();

    cloned_exec
        .spawn(async {
            loop {
                Timer::after(Duration::from_secs(150)).await;
                unsafe { crate::esp32::esp_idf_svc::sys::esp_task_wdt_reset() };
            }
        })
        .detach();

    let _ = cloned_exec.block_on(Box::pin(serve_async(
        exec,
        #[cfg(feature = "provisioning")]
        info,
        storage,
        repr,
        max_webrtc_connection,
    )));

    unreachable!()
}

#[cfg(feature = "provisioning")]
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
    // set the TWDT to expire after 5 minutes
    crate::esp32::esp_idf_svc::sys::esp!(unsafe {
        crate::esp32::esp_idf_svc::sys::esp_task_wdt_init(300, true)
    })
    .unwrap();

    // Register the current task on the TWDT. The TWDT runs in the IDLE Task.
    crate::esp32::esp_idf_svc::sys::esp!(unsafe {
        crate::esp32::esp_idf_svc::sys::esp_task_wdt_add(
            crate::esp32::esp_idf_svc::sys::xTaskGetCurrentTaskHandle(),
        )
    })
    .unwrap();

    let exec = Esp32Executor::new();
    let cloned_exec = exec.clone();

    cloned_exec
        .spawn(async {
            loop {
                Timer::after(Duration::from_secs(150)).await;
                unsafe { crate::esp32::esp_idf_svc::sys::esp_task_wdt_reset() };
            }
        })
        .detach();

    let _ = cloned_exec.block_on(Box::pin(serve_async_with_external_network(
        exec,
        info,
        storage,
        repr,
        network,
        max_webrtc_connection,
    )));

    unreachable!()
}
