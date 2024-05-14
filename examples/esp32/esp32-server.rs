#[cfg(target_os = "espidf")]
mod esp32 {
    #[allow(dead_code)]
    #[cfg(not(feature = "qemu"))]
    const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
    #[allow(dead_code)]
    #[cfg(not(feature = "qemu"))]
    const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

    include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

    #[cfg(not(feature = "provisioning"))]
    use log::*;
    use micro_rdk::common::entry::RobotRepresentation;
    #[cfg(feature = "qemu")]
    use micro_rdk::esp32::esp_idf_svc::eth::EspEth;
    #[cfg(not(feature = "provisioning"))]
    use micro_rdk::esp32::esp_idf_svc::eventloop::EspSystemEventLoop;
    use micro_rdk::esp32::esp_idf_svc::sys::{
        g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT,
    };
    #[cfg(feature = "qemu")]
    use std::net::Ipv4Addr;

    extern "C" {
        pub static g_spiram_ok: bool;
    }

    use micro_rdk::common::registry::ComponentRegistry;
    #[cfg(not(feature = "provisioning"))]
    use micro_rdk::esp32::esp_idf_svc::sys::EspError;

    #[cfg(all(not(feature = "qemu"), not(feature = "provisioning")))]
    use {
        embedded_svc::wifi::{
            AuthMethod, ClientConfiguration as WifiClientConfiguration,
            Configuration as WifiConfiguration,
        },
        micro_rdk::esp32::esp_idf_svc::hal::{peripheral::Peripheral, prelude::Peripherals},
        micro_rdk::esp32::esp_idf_svc::sys::esp_wifi_set_ps,
        micro_rdk::esp32::esp_idf_svc::wifi::{BlockingWifi, EspWifi},
    };

    pub(crate) fn main_esp32() {
        micro_rdk::esp32::esp_idf_svc::sys::link_patches();

        micro_rdk::esp32::esp_idf_svc::log::EspLogger::initialize_default();

        #[cfg(all(not(feature = "qemu"), not(feature = "provisioning")))]
        let periph = Peripherals::take().unwrap();

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
        let (ip, _block_eth) = {
            use micro_rdk::esp32::esp_idf_svc::hal::prelude::Peripherals;
            info!("creating eth object");
            let eth = micro_rdk::esp32::esp_idf_svc::eth::EspEth::wrap(
                micro_rdk::esp32::esp_idf_svc::eth::EthDriver::new_openeth(
                    Peripherals::take().unwrap().mac,
                    sys_loop_stack.clone(),
                )
                .unwrap(),
            )
            .unwrap();
            let (_, eth) = eth_configure(&sys_loop_stack, eth).unwrap();
            let ip = Ipv4Addr::new(10, 1, 12, 187);
            (ip, eth)
        };

        let mut max_connection = 3;
        unsafe {
            if !g_spiram_ok {
                log::info!("spiram not initialized disabling cache feature of the wifi driver");
                g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
                max_connection = 1;
            }
        }

        #[cfg(not(feature = "provisioning"))]
        {
            let sys_loop_stack = EspSystemEventLoop::take().unwrap();
            #[allow(clippy::redundant_clone)]
            #[cfg(not(feature = "qemu"))]
            let (ip, _wifi) = {
                let wifi = start_wifi(periph.modem, sys_loop_stack).expect("failed to start wifi");
                (
                    wifi.wifi()
                        .sta_netif()
                        .get_ip_info()
                        .expect("failed to get ip info")
                        .ip,
                    wifi,
                )
            };
            use micro_rdk::{
                common::app_client::AppClientConfig,
                esp32::{certificate::WebRtcCertificate, tls::Esp32TLSServerConfig},
            };
            let cfg = AppClientConfig::new(
                ROBOT_SECRET.to_owned(),
                ROBOT_ID.to_owned(),
                ip,
                "".to_owned(),
            );
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
                ip,
                webrtc_certificate,
                max_connection,
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
                std::net::Ipv4Addr::new(10, 1, 12, 187),
                max_connection,
            );
        }
    }

    #[cfg(feature = "qemu")]
    use micro_rdk::esp32::esp_idf_svc::eth::BlockingEth;
    #[cfg(feature = "qemu")]
    fn eth_configure<'d, T>(
        sl_stack: &EspSystemEventLoop,
        eth: micro_rdk::esp32::esp_idf_svc::eth::EspEth<'d, T>,
    ) -> Result<(Ipv4Addr, Box<BlockingEth<EspEth<'d, T>>>), EspError> {
        let mut eth = micro_rdk::esp32::esp_idf_svc::eth::BlockingEth::wrap(eth, sl_stack.clone())?;
        eth.start()?;
        eth.wait_netif_up()?;

        let ip_info = eth.eth().netif().get_ip_info()?;

        info!("ETH IP {:?}", ip_info.ip);
        Ok((ip_info.ip, Box::new(eth)))
    }

    #[cfg(all(not(feature = "qemu"), not(feature = "provisioning")))]
    fn start_wifi(
        modem: impl Peripheral<P = micro_rdk::esp32::esp_idf_svc::hal::modem::Modem> + 'static,
        sl_stack: EspSystemEventLoop,
    ) -> Result<Box<BlockingWifi<EspWifi<'static>>>, EspError> {
        let nvs = micro_rdk::esp32::esp_idf_svc::nvs::EspDefaultNvsPartition::take()?;
        let mut wifi =
            BlockingWifi::wrap(EspWifi::new(modem, sl_stack.clone(), Some(nvs))?, sl_stack)?;
        let wifi_configuration = WifiConfiguration::Client(WifiClientConfiguration {
            ssid: SSID.try_into().unwrap(),
            bssid: None,
            auth_method: AuthMethod::WPA2Personal,
            password: PASS.try_into().unwrap(),
            channel: None,
        });

        wifi.set_configuration(&wifi_configuration)?;

        wifi.start()?;
        info!("Wifi started");

        wifi.connect()?;
        info!("Wifi connected");

        wifi.wait_netif_up()?;
        info!("Wifi netif up");

        micro_rdk::esp32::esp_idf_svc::sys::esp!(unsafe {
            esp_wifi_set_ps(micro_rdk::esp32::esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
        })?;
        Ok(Box::new(wifi))
    }
}

fn main() {
    #[cfg(target_os = "espidf")]
    esp32::main_esp32();
}
