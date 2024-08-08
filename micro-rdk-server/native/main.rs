#[cfg(not(target_os = "espidf"))]
mod native {
    const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
    const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");

    use micro_rdk::{
        common::{
            conn::network::ExternallyManagedNetwork, credentials_storage::RAMStorage,
            entry::RobotRepresentation, provisioning::ProvisioningInfo,
        },
        native::entry::serve_web_with_external_network,
    };

    pub(crate) fn main_native() {
        env_logger::builder()
            .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
            .init();

        let repr = RobotRepresentation::WithRegistry(Box::default());

        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };

        let storage = if cfg!(has_robot_config) {
            RAMStorage::new(
                "",
                "",
                ROBOT_ID.expect("robot config missing ID"),
                ROBOT_SECRET.expect("robot config missing secret"),
            )
        } else {
            RAMStorage::default()
        };

        let info = if cfg!(feature = "provisioning") {
            let mut info = ProvisioningInfo::default();
            info.set_manufacturer("viam".to_owned());
            info.set_model("test-esp32".to_owned());
            Some(info)
        } else {
            None
        };

        serve_web_with_external_network(info, repr, 3, storage, network);
    }
}

fn main() {
    #[cfg(not(target_os = "espidf"))]
    {
        native::main_native();
    }
}
