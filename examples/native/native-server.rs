// Generated robot config during build process
include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

use log::*;
use micro_rdk::common::app_client::AppClientConfig;
use micro_rdk::common::robot::{Initializer, LocalRobot};
use micro_rdk::common::robot::ResourceType;
use micro_rdk::native::entry::serve_web;
use micro_rdk::native::tls::NativeTlsServerConfig;
use micro_rdk::proto::common::v1::ResourceName;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
fn main() -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();

    let initializer = {
        use micro_rdk::common::analog::FakeAnalogReader;
        use micro_rdk::common::base::FakeBase;
        use micro_rdk::common::board::FakeBoard;
        #[cfg(feature = "camera")]
        use micro_rdk::common::camera::FakeCamera;
        use micro_rdk::common::encoder::FakeEncoder;
        use micro_rdk::common::motor::FakeMotor;
        use micro_rdk::common::movement_sensor::FakeMovementSensor;
        let motor = Arc::new(Mutex::new(FakeMotor::new()));
        let base = Arc::new(Mutex::new(FakeBase::new()));
        let board = Arc::new(Mutex::new(FakeBoard::new(vec![
            Rc::new(RefCell::new(FakeAnalogReader::new("A1".to_string(), 10))),
            Rc::new(RefCell::new(FakeAnalogReader::new("A2".to_string(), 20))),
        ])));
        let movement_sensor = Arc::new(Mutex::new(FakeMovementSensor::new()));
        let enc = Arc::new(Mutex::new(FakeEncoder::new()));
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
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "movement_sensor".to_string(),
                name: "ms".to_string(),
            },
            ResourceType::MovementSensor(movement_sensor),
        );
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "encoder".to_string(),
                name: "enc".to_string(),
            },
            ResourceType::Encoder(enc),
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
        Initializer::WithRobot(LocalRobot::new(res))
    };

    let ip = match local_ip_address::local_ip().unwrap() {
        std::net::IpAddr::V4(ip) => ip,
        _ => panic!("ouups expected ipv4"),
    };

    let cfg = {
        let cert = ROBOT_SRV_PEM_CHAIN;
        let key = ROBOT_SRV_DER_KEY;
        NativeTlsServerConfig::new(cert.to_vec(), key.to_vec())
    };

    let app_config = AppClientConfig::new(
        ROBOT_SECRET.to_owned(),
        ROBOT_ID.to_owned(),
        ip,
        "".to_owned(),
    );

    serve_web(app_config, cfg, initializer, ip);

    Ok(())
}
