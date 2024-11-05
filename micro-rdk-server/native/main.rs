#[cfg(not(target_os = "espidf"))]
mod native {
    const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
    const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");
    const ROBOT_APP_ADDRESS: Option<&str> = option_env!("MICRO_RDK_ROBOT_APP_ADDRESS");

    use std::rc::Rc;

    use micro_rdk::{
        common::{
            conn::{
                network::{ExternallyManagedNetwork, Network},
                server::WebRtcConfiguration,
                viam::ViamServerBuilder,
            },
            credentials_storage::{RAMStorage, RobotConfigurationStorage, RobotCredentials},
            exec::Executor,
            log::initialize_logger,
            provisioning::server::ProvisioningInfo,
            registry::ComponentRegistry,
            webrtc::certificate::Certificate,
        },
        native::{
            certificate::WebRtcCertificate, conn::mdns::NativeMdns, dtls::NativeDtls,
            tcp::NativeH2Connector,
        },
    };

    pub(crate) fn main_native() {
        initialize_logger::<env_logger::Logger>();

        let network = match local_ip_address::local_ip().expect("error parsing local IP") {
            std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
            _ => panic!("oops expected ipv4"),
        };

        let registry = Box::<ComponentRegistry>::default();

        let storage = RAMStorage::new();

        // At runtime, if the program does not detect credentials or configs in storage,
        // it will try to load statically compiled values.

        if !storage.has_robot_configuration() {
            // check if any were statically compiled
            if ROBOT_ID.is_some() && ROBOT_SECRET.is_some() && ROBOT_APP_ADDRESS.is_some() {
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
                storage
                    .store_app_address(ROBOT_APP_ADDRESS.unwrap())
                    .expect("Failed to store app address")
            }
        }

        let mut info = ProvisioningInfo::default();
        info.set_manufacturer("viam".to_owned());
        info.set_model("test-esp32".to_owned());

        let webrtc_certs = Rc::new(Box::new(WebRtcCertificate::new()) as Box<dyn Certificate>);
        let dtls = Box::new(NativeDtls::new(webrtc_certs.clone()));
        let webrtc_config = WebRtcConfiguration::new(webrtc_certs, dtls);
        let mut builder = ViamServerBuilder::new(storage);
        let mdns = NativeMdns::new("".to_string(), network.get_ip()).unwrap();
        builder
            .with_http2_server(NativeH2Connector::default(), 12346)
            .with_webrtc_configuration(webrtc_config)
            .with_max_concurrent_connection(3)
            .with_provisioning_info(info)
            .with_component_registry(registry)
            .with_default_tasks();

        let mut server = builder.build(
            NativeH2Connector::default(),
            Executor::new(),
            mdns,
            Box::new(network),
        );
        server.run_forever::<bytes::BytesMut>();
    }
}

fn main() {
    #[cfg(not(target_os = "espidf"))]
    {
        native::main_native();
    }
}
