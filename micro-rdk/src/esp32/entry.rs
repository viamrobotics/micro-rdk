#![allow(dead_code)]

use std::{fmt::Debug, time::Duration};

use async_io::Timer;

use crate::{
    common::{
        app_client::AppClientError,
        conn::network::Network,
        credentials_storage::{RobotConfigurationStorage, WifiCredentialStorage},
        entry::validate_robot_credentials,
        entry::RobotRepresentation,
        grpc::ServerError,
        grpc_client::GrpcClientError,
        robot::LocalRobot,
    },
    esp32::{exec::Esp32Executor, tls::Esp32TLS},
    proto::app::v1::ConfigResponse,
};

#[cfg(feature = "provisioning")]
use crate::{
    common::provisioning::server::{serve_provisioning_async, ProvisioningInfo},
    esp32::conn::mdns::Esp32Mdns,
};

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
    use super::provisioning::wifi_provisioning::Esp32WifiProvisioningBuilder;

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

    crate::common::entry::serve_web_inner(
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
async fn serve_async_with_external_network<S>(
    exec: Esp32Executor,
    #[cfg(feature = "provisioning")] info: Option<ProvisioningInfo>,
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
    let mut client_connector = Esp32TLS::new_client();
    #[cfg(feature = "provisioning")]
    let info = info.unwrap_or_default();
    #[cfg(feature = "provisioning")]
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
                        log::info!("Unable to validate robot credentials {:?}; will retry", e);
                    }
                }
            }
        }

        #[cfg(feature = "provisioning")]
        {
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
        }
    };
    crate::common::entry::serve_web_inner(
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

pub fn serve_web_with_external_network<S>(
    #[cfg(feature = "provisioning")] info: Option<ProvisioningInfo>,
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
        #[cfg(feature = "provisioning")]
        info,
        storage,
        repr,
        network,
        max_webrtc_connection,
    )));

    unreachable!()
}
