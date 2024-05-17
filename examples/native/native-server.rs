#[cfg(not(target_os = "espidf"))]
mod native {
    #[allow(dead_code)]
    const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
    #[allow(dead_code)]
    const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");

    use micro_rdk::common::{
        conn::network::ExternallyManagedNetwork, entry::RobotRepresentation,
        provisioning::storage::RAMStorage,
    };
    use micro_rdk::native::entry::serve_web_with_external_network;

    pub(crate) fn main_native() {
        env_logger::builder()
            .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
            .init();

        let repr = RobotRepresentation::WithRegistry(Box::default());

        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };
        #[cfg(has_robot_config)]
        {
            if ROBOT_ID.is_some() && ROBOT_SECRET.is_some() {
                let ram_storage = RAMStorage::new("", "", ROBOT_ID.unwrap(), ROBOT_SECRET.unwrap());
                serve_web_with_external_network(None, repr, 3, ram_storage, network);
            }
            //TODO what?
        }
        #[cfg(not(has_robot_config))]
        {
            use micro_rdk::common::provisioning::server::ProvisioningInfo;
            let mut info = ProvisioningInfo::default();
            info.set_fragment_id("d385b480-3d19-4fad-a928-b5c18a58d0ed".to_string());
            info.set_manufacturer("viam".to_owned());
            info.set_model("test-esp32".to_owned());
            let storage = RAMStorage::default();
            log::info!("Will provision");
            serve_web_with_external_network(Some(info), repr, 3, storage, network);
        }
    }
}

fn main() {
    #[cfg(not(target_os = "espidf"))]
    {
        native::main_native();
    }
}
