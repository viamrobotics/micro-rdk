#![allow(dead_code)]

use std::{
    net::Ipv4Addr,
    rc::Rc,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
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
    robot::LocalRobot,
};

use super::{
    certificate::WebRtcCertificate,
    dtls::Esp32DtlsBuilder,
    exec::Esp32Executor,
    tcp::Esp32Stream,
    tls::{Esp32Tls, Esp32TlsServerConfig},
};

use futures_lite::Future;

pub async fn serve_web_inner(
    app_config: AppClientConfig,
    _tls_server_config: Esp32TlsServerConfig,
    repr: RobotRepresentation,
    _ip: Ipv4Addr,
    webrtc_certificate: WebRtcCertificate,
    exec: Esp32Executor<'_>,
    max_webrtc_connection: usize,
) {
    let (mut srv, robot) = {
        let mut client_connector = Esp32Tls::new_client();
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

            let mut client = builder.build().await.unwrap();

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
                            //TODO shouldn't panic here
                            panic!("couldn't build robot");
                        }
                    };
                    Arc::new(Mutex::new(r))
                }
            };

            (cfg_response, robot)
        };

        let webrtc_certificate = Rc::new(webrtc_certificate);
        let dtls = Esp32DtlsBuilder::new(webrtc_certificate.clone());

        let cloned_exec = exec.clone();

        let webrtc = Box::new(WebRtcConfiguration::new(
            webrtc_certificate,
            dtls,
            exec.clone(),
        ));

        (
            Box::new(
                ViamServerBuilder::new(
                    mdns,
                    cloned_exec,
                    client_connector,
                    app_config,
                    max_webrtc_connection,
                )
                .with_webrtc(webrtc)
                .build(&cfg_response)
                .unwrap(),
            ),
            robot,
        )
    };

    srv.serve(robot).await;
}

struct Esp32Waker;

impl Wake for Esp32Waker {
    fn wake(self: Arc<Self>) {}
    fn wake_by_ref(self: &Arc<Self>) {}
}
pub fn serve_web(
    app_config: AppClientConfig,
    tls_server_config: Esp32TlsServerConfig,
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

    let fut = cloned_exec.run(Box::pin(serve_web_inner(
        app_config,
        tls_server_config,
        repr,
        _ip,
        webrtc_certificate,
        exec,
        max_webrtc_connection,
    )));
    futures_lite::pin!(fut);

    let waker = Waker::from(Arc::new(Esp32Waker));

    let cx = &mut Context::from_waker(&waker);

    loop {
        match fut.as_mut().poll(cx) {
            Poll::Ready(_) => {
                unsafe { crate::esp32::esp_idf_svc::sys::esp_restart() };
            }
            Poll::Pending => {
                unsafe {
                    crate::esp32::esp_idf_svc::sys::esp_task_wdt_reset();
                    crate::esp32::esp_idf_svc::sys::vTaskDelay(10)
                };
            }
        }
    }
}
