use std::{
    net::Ipv4Addr,
    sync::{Arc, Mutex},
};

use {
    crate::common::conn::network::{Network, NetworkError},
    crate::esp32::esp_idf_svc::{
        eventloop::{EspSubscription, EspSystemEventLoop, System},
        hal::modem::Modem,
        sys::esp_wifi_set_ps,
        wifi::{BlockingWifi, EspWifi, WifiEvent},
    },
    embedded_svc::wifi::{
        AuthMethod, ClientConfiguration as WifiClientConfiguration,
        Configuration as WifiConfiguration,
    },
};

use crate::esp32::esp_idf_svc::{
    eth::{BlockingEth, EspEth, OpenEth},
    sys::EspError,
};

/// A wrapper around the wifi structure available in esp-idf-svc with and adjustment to support
/// reconnection
pub struct Esp32WifiNetwork {
    inner: Arc<Mutex<Box<BlockingWifi<EspWifi<'static>>>>>,
    sl_stack: EspSystemEventLoop,
    _subscription: Option<EspSubscription<'static, System>>,
}

impl Esp32WifiNetwork {
    pub fn new(
        sl_stack: EspSystemEventLoop,
        ssid: String,
        password: String,
    ) -> Result<Self, NetworkError> {
        let config = WifiConfiguration::Client(WifiClientConfiguration {
            ssid: ssid
                .as_str()
                .try_into()
                .expect("SSID to C string conversion failed"),
            bssid: None,
            auth_method: AuthMethod::WPA2Personal,
            password: password
                .as_str()
                .try_into()
                .expect("WiFi password to C string conversion failed"),
            channel: None,
        });
        let modem = unsafe { Modem::new() };
        let mut wifi = BlockingWifi::wrap(
            EspWifi::new(modem, sl_stack.clone(), None)?,
            sl_stack.clone(),
        )?;
        wifi.set_configuration(&config)?;
        let inner = Arc::new(Mutex::new(Box::new(wifi)));
        Ok(Self {
            inner,
            sl_stack,
            _subscription: None,
        })
    }
}

impl Esp32WifiNetwork {
    pub fn connect(&mut self) -> Result<(), NetworkError> {
        let inner_clone = self.inner.clone();
        let mut wifi = self.inner.lock().unwrap();
        wifi.start()?;
        log::info!("Wifi started");

        wifi.connect()?;
        log::info!("Wifi connected");

        wifi.wait_netif_up()?;
        log::info!("Wifi netif up");

        crate::esp32::esp_idf_svc::sys::esp!(unsafe {
            esp_wifi_set_ps(crate::esp32::esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
        })?;

        let subscription = self
            .sl_stack
            .subscribe::<WifiEvent, _>(move |event: WifiEvent| {
                if matches!(event, WifiEvent::StaDisconnected) {
                    let mut wifi_guard = inner_clone.lock().unwrap();
                    log::info!("wifi connecting...");
                    if let Err(err) = wifi_guard.wifi_mut().connect() {
                        log::error!("could not connect to wifi: {:?}", err);
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
        self.inner
            .lock()
            .unwrap()
            .wifi()
            .sta_netif()
            .get_ip_info()
            .expect("could not get IP info")
            .ip
    }
    fn is_connected(&self) -> Result<bool, NetworkError> {
        Ok(self.inner.lock().unwrap().is_connected()?)
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
