#![allow(dead_code)]

use std::{
    ffi::CString,
    fmt::Debug,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::common::{
    app_client::{AppClientBuilder, AppClientConfig},
    conn::{
        mdns::NoMdns,
        network::Network,
        server::{ViamServerBuilder, WebRtcConfiguration},
    },
    entry::RobotRepresentation,
    grpc_client::GrpcClient,
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

#[cfg(feature = "provisioning")]
use crate::common::{
    grpc::ServerError,
    provisioning::server::ProvisioningInfo,
    provisioning::storage::{RobotCredentialStorage, WifiCredentialStorage},
};

pub async fn serve_web_inner(
    robot_creds: RobotCredentials,
    repr: RobotRepresentation,
    exec: Esp32Executor,
    max_webrtc_connection: usize,
    network: impl Network,
) {
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
    // TODO: Spawn data task here. May have to move the initialization below to the task itself
    // TODO: Support implementers of the DataStore trait other than StaticMemoryDataStore in a way that is configurable
    {
        let _data_manager_svc = DataManager::<StaticMemoryDataStore>::from_robot_and_config(
            &cfg_response,
            &app_config,
            robot.clone(),
        );
    }

    let webrtc_certificate = Rc::new(webrtc_certificate);
    let dtls = Esp32DtlsBuilder::new(webrtc_certificate.clone());

    let cloned_exec = exec.clone();

    let webrtc = Box::new(WebRtcConfiguration::new(
        webrtc_certificate,
        dtls,
        exec.clone(),
    ));

    let mut srv = Box::new(
        ViamServerBuilder::new(
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
        })))
        .build(&cfg_response)
        .unwrap(),
    );

    srv.serve(robot).await;
}

async fn validate_robot_credentials(
    exec: Esp32Executor,
    robot_creds: &RobotCredentials,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_config = AppClientConfig::new(
        robot_creds.robot_secret().to_owned(),
        robot_creds.robot_id().to_owned(),
        "".to_owned(),
    );
    let conn = Esp32Stream::TLSStream(Box::new(Esp32TLS::new_client().open_ssl_context(None)?));
    let client = GrpcClient::new(conn, exec.clone(), "https://app.viam.com:443").await?;
    let builder = AppClientBuilder::new(Box::new(client), app_config.clone());

    let _client = builder.build().await?;
    Ok(())
}

// Four cases:
// 1) No Robot Credentials + WiFi without external network
// 2) No Robot Credentials with external network\
// 3) Robot Credentials with external network
// 4) Robot Credentials + WiFi without external network

#[cfg(feature = "provisioning")]
async fn serve_async<S>(
    exec: Esp32Executor,
    info: Option<ProvisioningInfo>,
    storage: S,
    repr: RobotRepresentation,
    max_webrtc_connection: usize,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotCredentialStorage>::Error: Debug,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    use crate::{
        common::provisioning::server::serve_provisioning_async,
        esp32::{
            conn::mdns::Esp32Mdns, provisioning::wifi_provisioning::Esp32WifiProvisioningBuilder,
        },
    };

    use super::conn::network::Esp32WifiNetwork;

    let mut last_error: Option<Box<dyn std::error::Error>> = None;

    let info = info.unwrap_or_default();

    let network = loop {
        // Credentials are present let's check we can connect
        if storage.has_stored_credentials() && storage.has_wifi_credentials() {
            let mut network =
                Esp32WifiNetwork::new(storage.get_wifi_credentials().unwrap()).await?;
            let validated = loop {
                // should check internet when implementing Cached Config
                let ret = network.connect().await;
                if let Err(error) = ret {
                    log::info!(
                        "Couldn't connect to {} cause : {:?}",
                        storage.get_wifi_credentials().unwrap().ssid,
                        error
                    );
                    continue;
                }
                // Assume connected to internet so any error should be forwarded to provisioning
                if let Err(e) = validate_robot_credentials(
                    exec.clone(),
                    &storage.get_robot_credentials().unwrap(),
                )
                .await
                {
                    let _ = last_error.insert(e);
                    break false;
                }
                break true;
            };
            if validated {
                break network;
            }
        }
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
    serve_web_inner(
        storage.get_robot_credentials().unwrap(),
        repr,
        exec,
        max_webrtc_connection,
        network,
    )
    .await;
    Ok(())
}

#[cfg(feature = "provisioning")]
async fn serve_async_with_external_network<S>(
    exec: Esp32Executor,
    info: Option<ProvisioningInfo>,
    storage: S,
    repr: RobotRepresentation,
    network: impl Network,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotCredentialStorage>::Error: Debug,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    use crate::common::provisioning::server::serve_provisioning_async;

    use super::conn::mdns::Esp32Mdns;

    let info = info.unwrap_or_default();
    let mut last_error: Option<Box<dyn std::error::Error>> = None;

    loop {
        // Credentials are present let's check we can connect
        if storage.has_stored_credentials() && storage.has_wifi_credentials() {
            let validated = loop {
                // should check internet when implementing Cached Config
                if let Err(error) = network.is_connected() {
                    log::info!(
                        "Externally managed network, not connected yet cause {:?}",
                        error
                    );
                    Timer::after(Duration::from_secs(3)).await;
                    continue;
                }
                // Assume connected to internet so any error should be forwarded to provisioning
                if let Err(e) = validate_robot_credentials(
                    exec.clone(),
                    &storage.get_robot_credentials().unwrap(),
                )
                .await
                {
                    let _ = last_error.insert(e);
                    break false;
                }
                break true;
            };
            if validated {
                break;
            }
        }
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
    }
    serve_web_inner(
        storage.get_robot_credentials().unwrap(),
        repr,
        exec,
        3,
        network,
    )
    .await;
    Ok(())
}

pub fn serve_web<S>(
    info: Option<ProvisioningInfo>,
    repr: RobotRepresentation,
    max_webrtc_connection: usize,
    storage: S,
) where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotCredentialStorage>::Error: Debug,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
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

    let e = cloned_exec.block_on(Box::pin(serve_async(
        exec,
        info,
        storage,
        repr,
        max_webrtc_connection,
    )));
    log::error!("Failed with {:?}", e);
    unreachable!()
}
