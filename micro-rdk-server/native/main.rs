#[cfg(not(target_os = "espidf"))]
mod native {
    const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
    const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");

    use micro_rdk::common::{
        conn::network::ExternallyManagedNetwork,
        credentials_storage::{RAMStorage, RobotConfigurationStorage, RobotCredentials},
        entry::{serve_with_network, RobotRepresentation},
        log::initialize_logger,
        provisioning::server::ProvisioningInfo,
        registry::ComponentRegistry,
    };

    pub(crate) fn main_native() {
        initialize_logger::<env_logger::Logger>();

        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };

        let registry = Box::<ComponentRegistry>::default();
        let repr = RobotRepresentation::WithRegistry(registry);

        let storage = RAMStorage::new();

        // At runtime, if the program does not detect credentials or configs in storage,
        // it will try to load statically compiled values.

        if !storage.has_robot_configuration() {
            // check if any were statically compiled
            if ROBOT_ID.is_some() && ROBOT_SECRET.is_some() {
                log::info!("Storing static values from build time robot configuration");
                storage
                    .store_robot_credentials(
                        RobotCredentials::new(
                            ROBOT_ID.unwrap().to_string(),
                            ROBOT_SECRET.unwrap().to_string(),
                        )
                        .into(),
                    )
                    .expect("Failed to store robot credentials");
            }
        }

        let max_connections = 3;

        let mut info = ProvisioningInfo::default();
        info.set_manufacturer("viam".to_owned());
        info.set_model("test-esp32".to_owned());

        serve_with_network(Some(info), repr, max_connections, storage, network);
    }
}

fn main() {
    #[cfg(not(target_os = "espidf"))]
    {
        native::main_native();
    }
}
