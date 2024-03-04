#[cfg(not(target_os = "espidf"))]
mod native {
    // Generated robot config during build process
    include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

    use micro_rdk::{
        common::{app_client::AppClientConfig, entry::RobotRepresentation},
        native::{entry::serve_web, tls::NativeTlsServerConfig},
    };

    pub(crate) fn main_native() -> anyhow::Result<()> {
        env_logger::init();

        let repr = RobotRepresentation::WithRegistry(Box::default());

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

        serve_web(app_config, cfg, repr, ip);

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    #[allow(unused_assignments, unused_mut)]
    let mut ret = Ok(());

    #[cfg(not(target_os = "espidf"))]
    {
        ret = native::main_native();
    }

    ret
}
