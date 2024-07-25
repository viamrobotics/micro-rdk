#![allow(dead_code)]

use std::{fmt::Debug, time::Duration};

use crate::{
    common::{
        conn::network::Network,
        credentials_storage::{RobotConfigurationStorage, WifiCredentialStorage},
        entry::RobotRepresentation,
        grpc::ServerError,
        robot::LocalRobot,
    },
    native::{conn::mdns::NativeMdns, exec::NativeExecutor, tls::NativeTls},
};

#[cfg(feature = "provisioning")]
use crate::{
    common::{
        app_client::AppClientError, entry::validate_robot_credentials,
        grpc_client::GrpcClientError, provisioning::server::ProvisioningInfo,
    },
    proto::app::v1::ConfigResponse,
};

#[cfg(feature = "provisioning")]
async fn serve_async_with_external_network<S>(
    exec: NativeExecutor,
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
    use async_io::Timer;

    use crate::common::provisioning::server::serve_provisioning_async;

    let mut client_connector = NativeTls::new_client();
    #[cfg(feature = "provisioning")]
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
                                log::warn!("Robot credential validation failed permanently with error {:?}; clearing cached state and initiating provisioning", e);

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
    use crate::common::app_client::{AppClientBuilder, AppClientConfig};

    use crate::common::grpc_client::GrpcClient;

    use crate::native::exec::NativeExecutor;
    use crate::native::tcp::NativeStream;
    use crate::native::tls::NativeTls;

    use futures_lite::future::block_on;

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
}
