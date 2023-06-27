#![allow(dead_code)]
use crate::common::app_client::{AppClientBuilder, AppClientConfig};
use crate::common::conn::server::{ViamServerBuilder, WebRtcConfiguration};
use crate::common::robot::LocalRobot;
use crate::{
    common::grpc_client::GrpcClient, esp32::exec::Esp32Executor, esp32::tcp::Esp32Stream,
    esp32::tls::Esp32Tls,
};

use std::net::{Ipv4Addr, SocketAddr};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::certificate::WebRtcCertificate;
use super::conn::mdns::Esp32Mdns;
use super::dtls::Esp32DtlsBuilder;
use super::tcp::Esp32Listener;
use super::tls::Esp32TlsServerConfig;

pub fn serve_web(
    app_config: AppClientConfig,
    tls_server_config: Esp32TlsServerConfig,
    robot: Option<LocalRobot>,
    _ip: Ipv4Addr,
    webrtc_certificate: WebRtcCertificate,
) {
    let (mut srv, robot) = {
        let mut client_connector = Esp32Tls::new_client();
        let exec = Esp32Executor::new();
        let mdns = Esp32Mdns::new("".to_string()).unwrap();

        let robot_cfg = {
            let cloned_exec = exec.clone();
            let conn = client_connector.open_ssl_context(None).unwrap();
            let conn = Esp32Stream::TLSStream(Box::new(conn));
            let grpc_client =
                Box::new(GrpcClient::new(conn, cloned_exec, "https://app.viam.com:443").unwrap());

            let builder = AppClientBuilder::new(grpc_client, app_config.clone());

            let mut client = builder.build().unwrap();

            client.get_config().unwrap()
        };

        let robot = match robot {
            Some(r) => Arc::new(Mutex::new(r)),
            None => {
                log::info!("building robot from config");
                let r = LocalRobot::new_from_config_response(&robot_cfg).unwrap();
                Arc::new(Mutex::new(r))
            }
        };

        let address: SocketAddr = "0.0.0.0:12346".parse().unwrap();
        let tls = Box::new(Esp32Tls::new_server(&tls_server_config));
        let tls_listener = Esp32Listener::new(address.into(), Some(tls)).unwrap();

        let webrtc_certificate = Rc::new(webrtc_certificate);
        let dtls = Esp32DtlsBuilder::new(webrtc_certificate.clone());

        let cloned_exec = exec.clone();

        let _ = handle_webhook(&*robot_cfg);

        let webrtc = Box::new(WebRtcConfiguration::new(
            webrtc_certificate,
            dtls,
            client_connector,
            exec.clone(),
            app_config,
        ));

        (
            Box::new(
                ViamServerBuilder::new(mdns, tls_listener, webrtc, cloned_exec, 12346)
                    .build(&robot_cfg)
                    .unwrap(),
            ),
            robot,
        )
    };

    srv.serve_forever(robot);
}

use crate::common::config::Component;
use crate::common::config::DynamicComponentConfig;
use crate::proto::app::v1::ConfigResponse;
use embedded_svc::{
    http::{client::Client as HttpClient, Method},
    io::Write,
    //utils::io,
};
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
use log::*;
use serde_json::json;
fn handle_webhook(robot_cfg: &ConfigResponse) -> anyhow::Result<()> {
    // webhook logic
    let robot_cfg = robot_cfg.clone();
    let components = &robot_cfg.config.as_ref().unwrap().components; // component config
    let cloud = robot_cfg.config.as_ref().unwrap().cloud.as_ref().unwrap(); //cloud config
    let fqdn = &cloud.fqdn; // robot's url
    let board_cfg: DynamicComponentConfig = components
        .iter()
        .find(|x| x.r#type == "board")
        .unwrap()
        .try_into()?;

    // if no webhook, return
    if let Ok(webhook) = board_cfg.get_attribute::<String>("webhook") {
        let secret = board_cfg.get_attribute::<String>("webhook-secret").unwrap_or("".to_string());
        let url = webhook.to_string();
        let payload = json!({
            "location": fqdn,
            "secret": secret,
            "board": board_cfg.name,
        })
        .to_string();
        info!("fqdn: {}", payload);

        let mut client = HttpClient::wrap(EspHttpConnection::new(&HttpConfiguration {
            crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
            ..Default::default()
        })?);

        let payload = payload.as_bytes();
        let headers = [
            ("accept", "text/plain"),
            ("content-type", "application/json"),
            ("connection", "close"),
            ("content-length", &format!("{}", payload.len())),
        ];
        let mut request = client.request(Method::Get, &url, &headers)?;
        request.write_all(payload)?;
        request.flush()?;
        info!("-> GET {}", url);
        let response = request.submit()?;
        info!("<- {}", response.status());
    }
    Ok(())
}
