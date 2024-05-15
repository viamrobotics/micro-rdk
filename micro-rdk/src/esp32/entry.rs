#![allow(dead_code)]

use std::{
    net::Ipv4Addr,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::common::{
    app_client::{AppClientBuilder, AppClientConfig},
    conn::{
        mdns::NoMdns,
        server::{ViamServerBuilder, WebRtcConfiguration},
    },
    entry::RobotRepresentation,
    grpc_client::GrpcClient,
    log::config_log_entry,
    restart_monitor::RestartMonitor,
    robot::LocalRobot,
};

#[cfg(feature = "data")]
use crate::common::{data_manager::DataManager, data_store::StaticMemoryDataStore};

use super::{
    certificate::WebRtcCertificate,
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
#[cfg(feature = "provisioning")]
use std::fmt::Debug;

pub async fn serve_web_inner(
    app_config: AppClientConfig,
    _tls_server_config: Esp32TLSServerConfig,
    repr: RobotRepresentation,
    _ip: Ipv4Addr,
    webrtc_certificate: WebRtcCertificate,
    exec: Esp32Executor,
    max_webrtc_connection: usize,
) {
    // TODO(NPM) this is a workaround so that async-io thread has started before we
    // instantiate the Async<TCPStream> for the connection to app.viam.com
    // otherwise there is a chance a race happens and will listen to events before full
    // initialization is done
    let _ = Timer::after(std::time::Duration::from_millis(60)).await;

    let mut client_connector = Esp32TLS::new_client();
    let mdns = NoMdns {};

    let (cfg_response, robot) = {
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

        let (cfg_response, cfg_received_datetime) = client.get_config().await.unwrap();

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

#[cfg(feature = "provisioning")]
pub fn serve_with_provisioning<S>(
    storage: S,
    info: ProvisioningInfo,
    repr: RobotRepresentation,
    ip: Ipv4Addr,
    max_webrtc_connection: usize,
) where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotCredentialStorage>::Error: Debug,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Send + Sync + 'static,
{
    use super::certificate::GeneratedWebRtcCertificateBuilder;

    let exec = Esp32Executor::new();
    let cloned_exec = exec.clone();
    let mut last_error = None;
    let (app_config, tls_server_config) = loop {
        match cloned_exec.block_on(Box::pin(
            crate::esp32::provisioning::server::serve_provisioning_async(
                ip,
                exec.clone(),
                info.clone(),
                storage.clone(),
                last_error.clone(),
            ),
        )) {
            Ok(c) => break c,
            Err(e) => {
                log::error!("provisioning step failed with {:?}", e);
                let _ = last_error.insert(format!("{:?}", e));
            }
        }
    };
    let webrtc_certificate = GeneratedWebRtcCertificateBuilder::default()
        .build()
        .unwrap();

    serve_web(
        app_config,
        tls_server_config,
        repr,
        ip,
        webrtc_certificate,
        max_webrtc_connection,
    );
    // unreachable!()
}

pub fn serve_web(
    app_config: AppClientConfig,
    tls_server_config: Esp32TLSServerConfig,
    repr: RobotRepresentation,
    _ip: Ipv4Addr,
    webrtc_certificate: WebRtcCertificate,
    max_webrtc_connection: usize,
) {
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

    cloned_exec.block_on(Box::pin(serve_web_inner(
        app_config,
        tls_server_config,
        repr,
        _ip,
        webrtc_certificate,
        exec,
        max_webrtc_connection,
    )));
    unreachable!()
}
