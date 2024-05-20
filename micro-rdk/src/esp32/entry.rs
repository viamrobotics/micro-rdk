#![allow(dead_code)]

use std::{
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
    provisioning::server::{ProvisioningInfo, ProvisioningServiceBuilder, ProvisoningServer},
    provisioning::storage::{RobotCredentialStorage, WifiCredentialStorage},
};
#[cfg(feature = "provisioning")]
use async_io::Async;
#[cfg(feature = "provisioning")]
use std::{fmt::Debug, net::TcpListener};

pub async fn serve_web_inner(
    app_config: AppClientConfig,
    _tls_server_config: Esp32TLSServerConfig,
    repr: RobotRepresentation,
    webrtc_certificate: WebRtcCertificate,
    exec: Esp32Executor,
    max_webrtc_connection: usize,
    network: impl Network,
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

#[cfg(feature = "provisioning")]
async fn serve_provisioning_async<S>(
    ip: std::net::Ipv4Addr,
    exec: Esp32Executor,
    info: ProvisioningInfo,
    storage: S,
    last_error: Option<String>,
) -> Result<(AppClientConfig, Esp32TLSServerConfig), Box<dyn std::error::Error>>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotCredentialStorage>::Error: Debug,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
{
    use super::conn::mdns::Esp32Mdns;
    use crate::common::conn::mdns::Mdns;
    use std::ffi::CString;
    let _ = Timer::after(std::time::Duration::from_millis(150)).await;
    let hostname = format!(
        "provisioning-{}-{}",
        info.get_model(),
        info.get_manufacturer()
    );
    let mut mdns = Esp32Mdns::new(hostname)?;

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

    mdns.add_service(
        "provisioning",
        "_rpc",
        "_tcp",
        port,
        &[("provisioning", "")],
    )?;
    loop {
        let incoming = listen.accept().await;
        let (stream, _) = incoming?;

        // The provisioning server is exposed over unencrypted HTTP2
        let stream = Esp32Stream::LocalPlain(stream);

        let ret = ProvisoningServer::new(srv.clone(), exec.clone(), stream).await;
        if ret.is_ok() && storage.has_stored_credentials() {
            let creds = storage.get_robot_credentials().unwrap();
            let app_config = AppClientConfig::new(
                creds.robot_secret().to_owned(),
                creds.robot_id().to_owned(),
                "".to_owned(),
            );

            let conn =
                Esp32Stream::TLSStream(Box::new(Esp32TLS::new_client().open_ssl_context(None)?));
            let client = GrpcClient::new(conn, exec.clone(), "https://app.viam.com:443").await?;
            let builder = AppClientBuilder::new(Box::new(client), app_config.clone());

            let mut client = builder.build().await?;
            let certs = client.get_certificates().await?;

            let serv_key = CString::new(certs.tls_private_key).unwrap();
            let serv_key_len = serv_key.as_bytes_with_nul().len() as u32;
            let serv_key: *const u8 = serv_key.into_raw() as *const u8;

            let tls_certs = CString::new(certs.tls_certificate)
                .unwrap()
                .into_bytes_with_nul();

            return Ok((
                app_config,
                Esp32TLSServerConfig::new(tls_certs, serv_key, serv_key_len),
            ));
        }
    }
}

#[cfg(feature = "provisioning")]
pub fn serve_with_provisioning<S>(
    storage: S,
    info: ProvisioningInfo,
    repr: RobotRepresentation,
    network: impl Network,
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
        match cloned_exec.block_on(Box::pin(serve_provisioning_async(
            network.get_ip(),
            exec.clone(),
            info.clone(),
            storage.clone(),
            last_error.clone(),
        ))) {
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
        webrtc_certificate,
        max_webrtc_connection,
        network,
    );
    unreachable!()
}

pub fn serve_web(
    app_config: AppClientConfig,
    tls_server_config: Esp32TLSServerConfig,
    repr: RobotRepresentation,
    webrtc_certificate: WebRtcCertificate,
    max_webrtc_connection: usize,
    network: impl Network,
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
        webrtc_certificate,
        exec,
        max_webrtc_connection,
        network,
    )));
    unreachable!()
}
