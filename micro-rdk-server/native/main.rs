#[cfg(not(target_os = "espidf"))]
mod native {
    #[allow(dead_code)]
    const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
    #[allow(dead_code)]
    const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");

    use micro_rdk::{
        common::{
            conn::network::ExternallyManagedNetwork, credentials_storage::RAMStorage,
            entry::RobotRepresentation,
        },
        native::entry::serve_web_with_external_network,
    };

    #[allow(unreachable_code)]
    pub(crate) fn main_native() {
        env_logger::builder()
            .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
            .init();

        #[allow(unused)]
        let repr = RobotRepresentation::WithRegistry(Box::default());

        #[allow(unused)]
        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };

        #[cfg(has_robot_config)]
        let storage = RAMStorage::new(
            "",
            "",
            ROBOT_ID.expect("robot config missing ID"),
            ROBOT_SECRET.expect("robot config missing secret"),
        );

        #[cfg(not(has_robot_config))]
        let storage = RAMStorage::default();

        #[cfg(feature = "provisioning")]
        {
            use micro_rdk::common::provisioning::server::ProvisioningInfo;
            let mut info = ProvisioningInfo::default();
            info.set_manufacturer("viam".to_owned());
            info.set_model("test-esp32".to_owned());
            serve_web_with_external_network(Some(info), repr, 3, network, storage);
        }

        #[cfg(not(feature = "provisioning"))]
        serve_web_with_external_network(repr, 3, storage, network);
    }
}

fn main() {
    #[cfg(not(target_os = "espidf"))]
    {
        native::main_native();
    }
}
