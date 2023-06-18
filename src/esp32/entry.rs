#![allow(dead_code)]
use crate::common::app_client::{AppClientBuilder, AppClientConfig};
use crate::common::conn::server::{ViamServerBuilder, WebRtcConfiguration};
use crate::common::robot::LocalRobot;
use crate::esp32::utils;
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
    utils::esp32_print_heap_internal_summary!();
    utils::esp32_print_stack_high_watermark!();
    let robot = Arc::new(Mutex::new(robot.unwrap()));

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

    utils::esp32_print_heap_internal_summary!();
    utils::esp32_print_stack_high_watermark!();
    let address: SocketAddr = "0.0.0.0:12346".parse().unwrap();
    let tls = Box::new(Esp32Tls::new_server(&tls_server_config));
    let tls_listener = Esp32Listener::new(address.into(), Some(tls)).unwrap();

    let webrtc_certificate = Rc::new(webrtc_certificate);
    let dtls = Esp32DtlsBuilder::new(webrtc_certificate.clone());

    let cloned_exec = exec.clone();

    let webrtc = Box::new(WebRtcConfiguration::new(
        webrtc_certificate,
        dtls,
        client_connector,
        exec.clone(),
        app_config,
    ));
    utils::esp32_print_heap_internal_summary!();
    utils::esp32_print_stack_high_watermark!();
    let mut srv = Box::new(
        ViamServerBuilder::new(mdns, tls_listener, webrtc, cloned_exec, 12346)
            .build(&robot_cfg)
            .unwrap(),
    );
    utils::esp32_print_heap_internal_summary!();
    utils::esp32_print_stack_high_watermark!();
    srv.serve_forever(robot);
}
