// Generated robot config during build process
include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

use log::*;
use micro_rdk::common::robot::LocalRobot;
use micro_rdk::common::robot::ResourceType;
use micro_rdk::native::server::{CloudConfig, NativeServer};
use micro_rdk::native::tls::NativeTlsServerConfig;
use micro_rdk::proto::common::v1::ResourceName;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

fn main() -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Debug)
        .init()
        .unwrap();
    // tracing_subscriber::fmt()
    //     // enable everything
    //     .with_max_level(tracing::Level::TRACE)
    //     // sets this to be the default, global collector for this application.
    //     .init();
    let robot = {
        use micro_rdk::common::analog::FakeAnalogReader;
        use micro_rdk::common::base::FakeBase;
        use micro_rdk::common::board::FakeBoard;
        #[cfg(feature = "camera")]
        use micro_rdk::common::camera::FakeCamera;
        use micro_rdk::common::motor::FakeMotor;
        let motor = Arc::new(Mutex::new(FakeMotor::new()));
        let base = Arc::new(Mutex::new(FakeBase::new()));
        let board = Arc::new(Mutex::new(FakeBoard::new(vec![
            Rc::new(RefCell::new(FakeAnalogReader::new("A1".to_string(), 10))),
            Rc::new(RefCell::new(FakeAnalogReader::new("A2".to_string(), 20))),
        ])));
        #[cfg(feature = "camera")]
        let camera = Arc::new(Mutex::new(FakeCamera::new()));
        let mut res: micro_rdk::common::robot::ResourceMap = HashMap::with_capacity(1);
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "motor".to_string(),
                name: "m1".to_string(),
            },
            ResourceType::Motor(motor),
        );
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "board".to_string(),
                name: "b".to_string(),
            },
            ResourceType::Board(board),
        );
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "base".to_string(),
                name: "base".to_string(),
            },
            ResourceType::Base(base),
        );
        #[cfg(feature = "camera")]
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "camera".to_string(),
                name: "c".to_string(),
            },
            ResourceType::Camera(camera),
        );
        LocalRobot::new(res)
    };

    let ip = match local_ip_address::local_ip().unwrap() {
        std::net::IpAddr::V4(ip) => ip,
        _ => panic!("ouups expected ipv4"),
    };

    let cfg = {
        let cert = include_bytes!(concat!(env!("OUT_DIR"), "/ca.crt"));
        let key = include_bytes!(concat!(env!("OUT_DIR"), "/key.key"));
        NativeTlsServerConfig::new(cert.to_vec(), key.to_vec())
    };
    let mut cloud_cfg = CloudConfig::new(ROBOT_NAME, LOCAL_FQDN, FQDN, ROBOT_ID, ROBOT_SECRET);
    cloud_cfg.set_tls_config(cfg);
    let esp32_srv = NativeServer::new(robot, cloud_cfg);
    esp32_srv.start(ip)?;
    Ok(())
}
