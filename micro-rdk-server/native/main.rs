#[cfg(not(target_os = "espidf"))]
mod native {
    #[allow(dead_code)]
    const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
    #[allow(dead_code)]
    const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");

    use micro_rdk::common::{conn::network::ExternallyManagedNetwork, entry::RobotRepresentation};

    #[allow(unreachable_code)]
    pub(crate) fn main_native() {
        env_logger::builder()
            .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
            .init();

        #[cfg(not(has_robot_config))]
        #[cfg(not(feature = "provisioning"))]
        {
            log::error!("cannot create robot without using robot config or provisioning");
            std::process::exit(1);
        }

        #[allow(unused)]
        let repr = RobotRepresentation::WithRegistry(Box::default());

        #[allow(unused)]
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
        }

        #[cfg(feature = "provisioning")]
        {
            use micro_rdk::{
                common::{credentials_storage::RAMStorage, provisioning::server::ProvisioningInfo},
                native::entry::serve_web_with_external_network,
            };

            let mut info = ProvisioningInfo::default();
            info.set_manufacturer("viam".to_owned());
            info.set_model("test-esp32".to_owned());
            let storage = RAMStorage::default();
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
