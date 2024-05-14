#[cfg(target_os = "espidf")]
mod esp32 {
    #[allow(dead_code)]
    #[cfg(not(feature = "qemu"))]
    const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
    #[allow(dead_code)]
    #[cfg(not(feature = "qemu"))]
    const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

    include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

    use micro_rdk::common::entry::RobotRepresentation;
    #[cfg(feature = "qemu")]
    use micro_rdk::esp32::conn::network::eth_configure;
    #[cfg(feature = "qemu")]
    use micro_rdk::esp32::esp_idf_svc::eth::{EspEth, EthDriver};
    use micro_rdk::esp32::esp_idf_svc::eventloop::EspSystemEventLoop;
    use micro_rdk::esp32::esp_idf_svc::sys::{
        g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT,
    };

    extern "C" {
        pub static g_spiram_ok: bool;
    }

    use micro_rdk::common::registry::ComponentRegistry;

    #[cfg(not(feature = "qemu"))]
    use micro_rdk::esp32::conn::network::Esp32WifiNetwork;

    pub(crate) fn main_esp32() {
        micro_rdk::esp32::esp_idf_svc::sys::link_patches();

        micro_rdk::esp32::esp_idf_svc::log::EspLogger::initialize_default();
        let sys_loop_stack = EspSystemEventLoop::take().unwrap();

        let repr = RobotRepresentation::WithRegistry(Box::<ComponentRegistry>::default());

        {
            micro_rdk::esp32::esp_idf_svc::sys::esp!(unsafe {
                micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_register(
                    &micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
                )
            })
            .unwrap();
        }

        #[cfg(feature = "qemu")]
        let network = {
            use micro_rdk::esp32::esp_idf_svc::hal::prelude::Peripherals;
            log::info!("creating eth object");
            let eth = EspEth::wrap(
                EthDriver::new_openeth(Peripherals::take().unwrap().mac, sys_loop_stack.clone())
                    .unwrap(),
            )
            .unwrap();
            eth_configure(&sys_loop_stack, eth).unwrap()
        };

        let mut max_connection = 3;
        unsafe {
            if !g_spiram_ok {
                log::info!("spiram not initialized disabling cache feature of the wifi driver");
                g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
                max_connection = 1;
            }
        }
        #[allow(clippy::redundant_clone)]
        #[cfg(not(feature = "qemu"))]
        let network = {
            let mut wifi =
                Esp32WifiNetwork::new(sys_loop_stack.clone(), SSID.to_string(), PASS.to_string())
                    .unwrap();
            loop {
                match wifi.connect() {
                    Ok(()) => break,
                    Err(_) => {
                        log::info!("wifi could not connect to SSID: {:?}, retrying...", SSID);
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                }
            }
            wifi
        };

        #[cfg(not(feature = "provisioning"))]
        {
            use micro_rdk::{
                common::app_client::AppClientConfig,
                esp32::{certificate::WebRtcCertificate, tls::Esp32TLSServerConfig},
            };
            let cfg =
                AppClientConfig::new(ROBOT_SECRET.to_owned(), ROBOT_ID.to_owned(), "".to_owned());
            let webrtc_certificate = WebRtcCertificate::new(
                ROBOT_DTLS_CERT.to_vec(),
                ROBOT_DTLS_KEY_PAIR.to_vec(),
                ROBOT_DTLS_CERT_FP,
            );

            let tls_cfg = {
                let cert = ROBOT_SRV_PEM_CHAIN.to_vec();
                let key = ROBOT_SRV_DER_KEY;
                Esp32TLSServerConfig::new(cert, key.as_ptr(), key.len() as u32)
            };

            micro_rdk::esp32::entry::serve_web(
                cfg,
                tls_cfg,
                repr,
                webrtc_certificate,
                max_connection,
                network,
            );
        }
        #[cfg(feature = "provisioning")]
        {
            use micro_rdk::common::provisioning::{server::ProvisioningInfo, storage::RAMStorage};
            let mut info = ProvisioningInfo::default();
            info.set_fragment_id("d385b480-3d19-4fad-a928-b5c18a58d0ed".to_string());
            info.set_manufacturer("viam".to_owned());
            info.set_model("test-esp32".to_owned());
            let storage = RAMStorage::default();
            micro_rdk::esp32::entry::serve_with_provisioning(
                storage,
                info,
                repr,
                network,
                max_connection,
            );
        }
    }
}

fn main() {
    #[cfg(target_os = "espidf")]
    esp32::main_esp32();
}
