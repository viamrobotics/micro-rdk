#![allow(dead_code)]

use std::{
    ffi::CString,
    fmt::Debug,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::common::{
    app_client::{AppClient, AppClientBuilder, AppClientConfig, AppClientError},
    conn::{
        mdns::NoMdns,
        network::Network,
        server::{ViamServerBuilder, WebRtcConfiguration},
    },
    entry::RobotRepresentation,
    grpc_client::{GrpcClient, GrpcClientError},
    log::config_log_entry,
    provisioning::storage::RobotCredentials,
    restart_monitor::RestartMonitor,
    robot::LocalRobot,
};

#[cfg(feature = "data")]
use crate::common::{data_manager::DataManager, data_store::StaticMemoryDataStore};

use super::{
    certificate::GeneratedWebRtcCertificateBuilder,
    dtls::Esp32DtlsBuilder,
    exec::Esp32Executor,
    tcp::Esp32Stream,
    tls::{Esp32TLS, Esp32TLSServerConfig},
};

use async_io::Timer;
use esp_idf_svc::sys::{settimeofday, timeval};

#[cfg(feature = "provisioning")]
use crate::common::{
    grpc::ServerError,
    provisioning::server::ProvisioningInfo,
    provisioning::storage::{RobotConfigurationStorage, WifiCredentialStorage},
};

pub async fn serve_web_inner<S>(
    storage: S,
    repr: RobotRepresentation,
    exec: Esp32Executor,
    max_webrtc_connection: usize,
    network: impl Network,
) where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    let robot_creds = storage
        .get_robot_credentials()
        .expect("serve_web_inner: called with storage lacking robot credentials");

    // TODO(NPM) this is a workaround so that async-io thread has started before we
    // instantiate the Async<TCPStream> for the connection to app.viam.com
    // otherwise there is a chance a race happens and will listen to events before full
    // initialization is done
    let _ = Timer::after(std::time::Duration::from_millis(60)).await;

    let webrtc_certificate = GeneratedWebRtcCertificateBuilder::default()
        .build()
        .unwrap();

    let app_config = AppClientConfig::new(
        robot_creds.robot_secret().to_owned(),
        robot_creds.robot_id().to_owned(),
        "".to_owned(),
    );

    let mut client_connector = Esp32TLS::new_client();
    let mdns = NoMdns {};

    let (cfg_response, robot, _tls_server_config) = {
        let cloned_exec = exec.clone();
        let conn = client_connector.open_ssl_context(None).unwrap();
        let conn = Esp32Stream::TLSStream(Box::new(conn));
        let grpc_client = Box::new(
            GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443")
                .await
                .unwrap(),
        );

        let builder = AppClientBuilder::new(grpc_client, app_config.clone());

        let client = builder.build().await.unwrap();

        let certs = client.get_certificates().await.unwrap();

        let serv_key = CString::new(certs.tls_private_key).unwrap();
        let serv_key_len = serv_key.as_bytes_with_nul().len() as u32;
        let serv_key: *const u8 = serv_key.into_raw() as *const u8;

        let tls_certs = CString::new(certs.tls_certificate)
            .unwrap()
            .into_bytes_with_nul();
        let tls_server_config = Esp32TLSServerConfig::new(tls_certs, serv_key, serv_key_len);

        let (cfg_response, cfg_received_datetime) =
            client.get_config(network.get_ip()).await.unwrap();

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

        (cfg_response, robot, tls_server_config)
    };

    #[cfg(feature = "data")]
    // TODO: Support implementers of the DataStore trait other than StaticMemoryDataStore in a way that is configurable
    let data_manager_svc = match DataManager::<StaticMemoryDataStore>::from_robot_and_config(
        &cfg_response,
        &app_config,
        robot.clone(),
    ) {
        Ok(svc) => svc,
        Err(err) => {
            log::error!("error configuring data management: {:?}", err);
            None
        }
    };

    #[cfg(feature = "data")]
    let data_sync_task = data_manager_svc
        .as_ref()
        .map(|data_manager_svc| data_manager_svc.get_sync_task());

    #[cfg(feature = "data")]
    let data_future = Box::pin(async move {
        if let Some(mut data_manager_svc) = data_manager_svc {
            if let Err(err) = data_manager_svc.data_collection_task().await {
                log::error!("error running data manager: {:?}", err)
            }
        }
    });
    #[cfg(not(feature = "data"))]
    let data_future = async move {};

    let webrtc_certificate = Rc::new(webrtc_certificate);
    let dtls = Esp32DtlsBuilder::new(webrtc_certificate.clone());

    let cloned_exec = exec.clone();

    let webrtc = Box::new(WebRtcConfiguration::new(
        webrtc_certificate,
        dtls,
        exec.clone(),
    ));

    let mut srv = {
        let builder = ViamServerBuilder::new(
            mdns,
            cloned_exec,
            client_connector,
            app_config,
            max_webrtc_connection,
            network,
        )
        .with_webrtc(webrtc)
        .with_periodic_app_client_task(Box::new(RestartMonitor::new(|| unsafe {
            crate::esp32::esp_idf_svc::sys::esp_restart()
        })));
        #[cfg(feature = "data")]
        let builder = if let Some(task) = data_sync_task {
            builder.with_periodic_app_client_task(Box::new(task))
        } else {
            builder
        };
        builder.build(&cfg_response).unwrap()
    };

    // Attempt to cache the config for the machine we are about to `serve`.
    if let Err(e) = storage.store_robot_configuration(cfg_response.config.unwrap()) {
        log::warn!("Failed to store robot configuration: {}", e);
    }

    futures_lite::future::zip(Box::pin(srv.serve(robot)), data_future).await;
}

async fn validate_robot_credentials(
    exec: Esp32Executor,
    robot_creds: &RobotCredentials,
) -> Result<AppClient, Box<dyn std::error::Error>> {
    let app_config = AppClientConfig::new(
        robot_creds.robot_secret().to_owned(),
        robot_creds.robot_id().to_owned(),
        "".to_owned(),
    );
    let conn = Esp32Stream::TLSStream(Box::new(Esp32TLS::new_client().open_ssl_context(None)?));
    let client = GrpcClient::new(conn, exec.clone(), "https://app.viam.com:443").await?;
    let builder = AppClientBuilder::new(Box::new(client), app_config.clone());

    builder.build().await.map_err(|e| e.into())
}

// Four cases:
// 1) No Robot Credentials + WiFi without external network
// 2) No Robot Credentials with external network\
// 3) Robot Credentials with external network
// 4) Robot Credentials + WiFi without external network
// The function attempts to connect to the configured Wifi network if any, it then checks the robot credentials. If Wifi credentials are absent it starts provisioning mode
// If they are invalid or absent it will start the provisioning server. Once provision is done it invokes the main server.
#[cfg(feature = "provisioning")]
async fn serve_async<S>(
    exec: Esp32Executor,
    info: Option<ProvisioningInfo>,
    storage: S,
    repr: RobotRepresentation,
    max_webrtc_connection: usize,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    use crate::{
        common::provisioning::server::serve_provisioning_async,
        esp32::{
            conn::mdns::Esp32Mdns, provisioning::wifi_provisioning::Esp32WifiProvisioningBuilder,
        },
    };

    use super::conn::network::Esp32WifiNetwork;

    let info = info.unwrap_or_default();
    let mut last_error: Option<Box<dyn std::error::Error>> = None;

    let (robot, network, app_client) = 'provisioned: loop {
        if storage.has_robot_credentials() && storage.has_wifi_credentials() {
            log::info!("Found cached network and robot credentials; attempting to serve");

            let mut robot = None;
            if storage.has_robot_configuration() {
                log::info!("Found cached robot configuration; speculatively building robot");
                let _ = robot.insert(());
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
                                log::warn!("Couldn't determine network connectivity due to {:?}; initiating provisioning", e);
                                let _ = last_error.insert(e.into());
                                break;
                            }
                        }

                        log::info!("Attempting to validate stored robot credentials");
                        match validate_robot_credentials(
                            exec.clone(),
                            &storage.get_robot_credentials().unwrap(),
                        )
                        .await
                        {
                            Ok(app_client) => {
                                log::info!("Robot credentials validated OK");
                                break 'provisioned (robot, network, app_client);
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

                                        // Record the last error so that we can serve it once we reach provisioning.
                                        let _ = last_error.insert(e);
                                        break;
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
                    log::info!(
                        "Unable to create network with cached credentials; initiating provisioning"
                    );
                    // If we can't even construct the network
                    // with the cached wifi credentials, fall
                    // back to provisioning.
                    let _ = last_error.insert(e.into());
                }
            };
        }

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
    };

    serve_web_inner(storage, repr, exec, max_webrtc_connection, network).await;
    Ok(())
}

// serve_async variant where an external network is provided
#[cfg(feature = "provisioning")]
async fn serve_async_with_external_network<S>(
    exec: Esp32Executor,
    info: Option<ProvisioningInfo>,
    storage: S,
    repr: RobotRepresentation,
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

    let info = info.unwrap_or_default();
    let mut last_error: Option<Box<dyn std::error::Error>> = None;

    let (robot, app_client) = 'provisioned: loop {
        if storage.has_robot_credentials() {
            log::info!("Found cached robot credentials; attempting to serve");

            let mut robot = None;
            if storage.has_robot_configuration() {
                log::info!("Found cached robot configuration; speculatively building robot");
                let _ = robot.insert(());
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
                )
                .await
                {
                    Ok(app_client) => {
                        log::info!("Robot credentials validated OK");
                        break 'provisioned (robot, app_client);
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
    serve_web_inner(storage, repr, exec, max_webrtc_connection, network).await;
    Ok(())
}

pub fn serve_web<S>(
    info: Option<ProvisioningInfo>,
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
        info,
        storage,
        repr,
        max_webrtc_connection,
    )));

    unreachable!()
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
