use std::net::Ipv4Addr;

use {
    crate::common::conn::network::{Network, NetworkError},
    crate::esp32::esp_idf_svc::{
        eventloop::{EspSubscription, EspSystemEventLoop, System},
        sys::esp_wifi_set_ps,
        wifi::{EspWifi, WifiEvent},
    },
    embedded_svc::wifi::{
        AuthMethod, ClientConfiguration as WifiClientConfiguration,
        Configuration as WifiConfiguration,
    },
};

use esp_idf_svc::{hal::modem::WifiModem, timer::EspTaskTimerService, wifi::AsyncWifi};
use futures_util::lock::Mutex;
use once_cell::sync::OnceCell;

use crate::{
    common::provisioning::storage::WifiCredentials,
    esp32::esp_idf_svc::{
        eth::{BlockingEth, EspEth, OpenEth},
        sys::EspError,
    },
};

pub(crate) fn esp32_get_system_event_loop() -> Result<&'static EspSystemEventLoop, EspError> {
    static INSTANCE: OnceCell<EspSystemEventLoop> = OnceCell::new();
    INSTANCE.get_or_try_init(EspSystemEventLoop::take)
}
pub(crate) fn esp32_get_wifi() -> Result<&'static Mutex<AsyncWifi<EspWifi<'static>>>, EspError> {
    static INSTANCE: OnceCell<Mutex<AsyncWifi<EspWifi<'static>>>> = OnceCell::new();
    INSTANCE.get_or_try_init(|| {
        // Wifi shouldn't be already instantiated Does esp have a function to check status?
        let modem = unsafe { WifiModem::new() };

        let sys_loop = esp32_get_system_event_loop()?;

        let wifi = EspWifi::new(modem, sys_loop.clone(), None)?;

        let task_timer = EspTaskTimerService::new()?;

        let wifi = AsyncWifi::wrap(wifi, sys_loop.clone(), task_timer)?;
        Ok(Mutex::new(wifi))
    })
}

/// A wrapper around the wifi structure available in esp-idf-svc with and adjustment to support
/// reconnection
pub struct Esp32WifiNetwork {
    _subscription: Option<EspSubscription<'static, System>>,
}

impl Esp32WifiNetwork {
    pub async fn new(wifi_creds: WifiCredentials) -> Result<Self, NetworkError> {
        let config = WifiConfiguration::Client(WifiClientConfiguration {
            ssid: wifi_creds
                .ssid
                .as_str()
                .try_into()
                .map_err(|_| NetworkError::HeapLessStringConversionError)?,
            auth_method: AuthMethod::None,
            password: wifi_creds
                .pwd
                .as_str()
                .try_into()
                .map_err(|_| NetworkError::HeapLessStringConversionError)?,
            ..Default::default()
        });
        let mut wifi = esp32_get_wifi()?.lock().await;

        crate::esp32::esp_idf_svc::sys::esp!(unsafe {
            esp_wifi_set_ps(crate::esp32::esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
        })?;

        wifi.stop().await?;
        wifi.set_configuration(&config)?;

        Ok(Self {
            _subscription: None,
        })
    }
}

impl Esp32WifiNetwork {
    pub async fn connect(&mut self) -> Result<(), NetworkError> {
        let mut wifi = esp32_get_wifi()?.lock().await;
        wifi.start().await?;
        wifi.connect().await?;
        wifi.wait_netif_up().await?;

        let sl_stack = esp32_get_system_event_loop()?;

        let subscription = sl_stack.subscribe::<WifiEvent, _>(move |event: WifiEvent| {
            if matches!(event, WifiEvent::StaDisconnected) {
                if let Ok(wifi) = esp32_get_wifi() {
                    if let Some(mut wifi_guard) = wifi.try_lock() {
                        let wifi_mut = wifi_guard.wifi_mut();
                        if let Err(err) = wifi_mut.connect() {
                            let ssid = wifi_mut
                                .get_configuration()
                                .map_or("<no_ssid>".to_owned(), |c| {
                                    c.as_client_conf_ref().unwrap().ssid.to_string()
                                });
                            log::error!("could not connect to WiFi {} cause : {:?}", ssid, err);
                        }
                    }
                }
            } else if matches!(event, WifiEvent::StaConnected) {
                log::info!("wifi connected event received");
            }
        })?;
        let _ = self._subscription.replace(subscription);
        Ok(())
    }
}

impl Network for Esp32WifiNetwork {
    fn get_ip(&self) -> Ipv4Addr {
        let guard = esp32_get_wifi().map_or(None, |wifi| wifi.try_lock());

        guard.map_or(Ipv4Addr::UNSPECIFIED, |guard| {
            guard
                .wifi()
                .sta_netif()
                .get_ip_info()
                .map_or(Ipv4Addr::UNSPECIFIED, |ip_info| ip_info.ip)
        })
    }
    fn is_connected(&self) -> Result<bool, NetworkError> {
        let guard = esp32_get_wifi().map_or(None, |wifi| wifi.try_lock());
        Ok(guard.map_or(Ok(false), |guard| guard.is_connected())?)
    }
}

pub fn eth_configure<'d, T>(
    sl_stack: &EspSystemEventLoop,
    eth: EspEth<'d, T>,
) -> Result<Box<BlockingEth<EspEth<'d, T>>>, EspError> {
    let mut eth = BlockingEth::wrap(eth, sl_stack.clone())?;
    eth.start()?;
    eth.wait_netif_up()?;
    Ok(Box::new(eth))
}

impl Network for Box<BlockingEth<EspEth<'static, OpenEth>>> {
    fn get_ip(&self) -> Ipv4Addr {
        self.eth()
            .netif()
            .get_ip_info()
            .expect("could not get IP info")
            .ip
    }
    fn is_connected(&self) -> Result<bool, NetworkError> {
        Ok(BlockingEth::is_connected(self)?)
    }
}
