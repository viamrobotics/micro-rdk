#![allow(dead_code)]

use std::{
    net::{Ipv4Addr, SocketAddr},
    rc::Rc,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
};

use crate::common::{
    app_client::{AppClientBuilder, AppClientConfig},
    conn::server::{ViamServerBuilder, WebRtcConfiguration},
    entry::RobotRepresentation,
    grpc_client::GrpcClient,
    log::config_log_entry,
    robot::LocalRobot,
};

use super::{
    certificate::WebRtcCertificate,
    conn::mdns::Esp32Mdns,
    dtls::Esp32DtlsBuilder,
    exec::Esp32Executor,
    tcp::{Esp32Listener, Esp32Stream},
    tls::{Esp32Tls, Esp32TlsServerConfig},
    webhook::Webhook,
};

use embedded_svc::http::client::Client as HttpClient;
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
use futures_lite::Future;

pub async fn serve_web_inner(
    app_config: AppClientConfig,
    tls_server_config: Esp32TlsServerConfig,
    repr: RobotRepresentation,
    _ip: Ipv4Addr,
    webrtc_certificate: WebRtcCertificate,
    exec: Esp32Executor<'_>,
) {
    let (mut srv, robot) = {
        let mut client_connector = Esp32Tls::new_client();
        let mdns = Esp32Mdns::new("".to_string()).unwrap();

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
                                let logs = vec![config_log_entry(datetime, Some(&err))];
                                client
                                    .push_logs(logs)
                                    .await
                                    .expect("could not push logs to app");
                            }
                            panic!("{}", err)
                        }
                    };
                    Arc::new(Mutex::new(r))
                }
            };

            (cfg_response, robot)
        };

        let address: SocketAddr = "0.0.0.0:12346".parse().unwrap();
        let tls = Box::new(Esp32Tls::new_server(&tls_server_config));
        let tls_listener = Esp32Listener::new(address.into(), Some(tls)).unwrap();

        let webrtc_certificate = Rc::new(webrtc_certificate);
        let dtls = Esp32DtlsBuilder::new(webrtc_certificate.clone());

        let cloned_exec = exec.clone();

        let webrtc = Box::new(WebRtcConfiguration::new(
            webrtc_certificate,
            dtls,
            exec.clone(),
        ));

        let robot_cfg = cfg_response.as_ref().config.as_ref().unwrap();

        if let Ok(webhook) = Webhook::from_robot_config(robot_cfg) {
            if webhook.has_endpoint() {
                // only make a client if a webhook url is present
                let mut client = HttpClient::wrap(
                    EspHttpConnection::new(&HttpConfiguration {
                        crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
                        ..Default::default()
                    })
                    .unwrap(),
                );

                let _ = webhook.send(&mut client);
            }
        }

        (
            Box::new(
                ViamServerBuilder::new(mdns, cloned_exec, client_connector, app_config)
                    .with_webrtc(webrtc)
                    .with_http2(tls_listener, 12346)
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
) {
    esp_idf_sys::esp!(unsafe { esp_idf_sys::esp_task_wdt_init(300, true) }).unwrap();

    esp_idf_sys::esp!(unsafe {
        esp_idf_sys::esp_task_wdt_add(esp_idf_sys::xTaskGetCurrentTaskHandle())
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
    )));
    futures_lite::pin!(fut);

    let waker = Waker::from(Arc::new(Esp32Waker));

    let cx = &mut Context::from_waker(&waker);

    loop {
        match fut.as_mut().poll(cx) {
            Poll::Ready(_) => {
                unsafe { esp_idf_sys::esp_restart() };
            }
            Poll::Pending => {
                unsafe {
                    esp_idf_sys::esp_task_wdt_reset();
                    esp_idf_sys::vTaskDelay(10)
                };
            }
        }
    }
}
