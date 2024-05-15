#[cfg(not(target_os = "espidf"))]
mod native {
    // Generated robot config during build process
    include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

    use micro_rdk::common::{conn::network::ExternallyManagedNetwork, entry::RobotRepresentation};

    pub(crate) fn main_native() {
        env_logger::builder()
            .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
            .init();

        let repr = RobotRepresentation::WithRegistry(Box::default());

        let network = match local_ip_address::local_ip().unwrap() {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };
        #[cfg(not(feature = "provisioning"))]
        {
            use micro_rdk::common::app_client::AppClientConfig;
            use micro_rdk::native::tls::NativeTlsServerConfig;
            let cfg = {
                let cert = ROBOT_SRV_PEM_CHAIN;
                let key = ROBOT_SRV_DER_KEY;
                NativeTlsServerConfig::new(cert.to_vec(), key.to_vec())
            };

            let app_config =
                AppClientConfig::new(ROBOT_SECRET.to_owned(), ROBOT_ID.to_owned(), "".to_owned());

            micro_rdk::native::entry::serve_web(app_config, cfg, repr, network);
        }
        #[cfg(feature = "provisioning")]
        {
            use micro_rdk::common::provisioning::{
                server::ProvisioningInfo, storage::MemoryCredentialStorage,
            };
            let mut info = ProvisioningInfo::default();
            info.set_fragment_id("d385b480-3d19-4fad-a928-b5c18a58d0ed".to_string());
            info.set_manufacturer("viam".to_owned());
            info.set_model("test".to_owned());
            let storage = MemoryCredentialStorage::default();
            micro_rdk::native::entry::serve_with_provisioning(storage, info, repr, ip);
        }
    }
}

fn main() {
    #[cfg(not(target_os = "espidf"))]
    {
        native::main_native();
    }
}
