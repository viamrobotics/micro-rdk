use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::WifiModem,
    sys::{self, EspError},
    timer::EspTaskTimerService,
    wifi::{
        AccessPointConfiguration, AccessPointInfo, AsyncWifi, AuthMethod, ClientConfiguration,
        Configuration, EspWifi, Protocol,
    },
};
use futures_util::lock::Mutex;
use once_cell::sync::OnceCell;
use std::{cell::RefCell, num::NonZeroI32};

use crate::common::provisioning::{
    server::{NetworkInfo, WifiManager},
    storage::{WifiCredentialStorage, WifiCredentials},
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

pub struct Esp32WifiProvisioning<S> {
    storage: S,
    config: RefCell<Configuration>,
}

impl From<&AccessPointInfo> for NetworkInfo {
    fn from(value: &AccessPointInfo) -> Self {
        let mut info = NetworkInfo::default();
        info.0.ssid = value.ssid.to_string();
        info.0.security = value
            .auth_method
            .map_or("none".to_owned(), |auth| auth.to_string());
        info.0.signal = value.signal_strength.into();
        info.0.r#type = "2.4GhZ".to_owned();
        info
    }
}

impl<S: WifiCredentialStorage> WifiManager for Esp32WifiProvisioning<S> {
    type Error = EspError;
    async fn scan_networks(
        &self,
    ) -> Result<Vec<crate::common::provisioning::server::NetworkInfo>, Self::Error> {
        let networks = self.scan_networks().await?;
        let networks = networks.iter().map(NetworkInfo::from).collect();
        Ok(networks)
    }
    async fn try_connect(&self, ssid: &str, password: &str) -> Result<(), Self::Error> {
        self.try_connect_to(ssid, password)
            .await
            .map_err(Into::into)
    }
}

impl<S: WifiCredentialStorage> Esp32WifiProvisioning<S> {
    pub async fn new(storage: S) -> Result<Self, EspError> {
        let mut mac_address = [0_u8; 8];
        unsafe {
            sys::esp!(sys::esp_efuse_mac_get_default(
                mac_address.as_mut_ptr() as *mut u8
            ))?;
        };
        let ap_conf = AccessPointConfiguration {
            ssid: format!("esp32-{:02X}-{:02X}", mac_address[4], mac_address[5])
                .as_str()
                .try_into()
                .unwrap(),
            ssid_hidden: false,
            channel: 10,
            secondary_channel: None,
            protocols: Protocol::P802D11B | Protocol::P802D11BG | Protocol::P802D11BGN,
            auth_method: esp_idf_svc::wifi::AuthMethod::WPA2Personal,
            password: "password".try_into().unwrap(),
            max_connections: 1,
        };
        let sta_conf = ClientConfiguration {
            ssid: "".try_into().unwrap(),
            bssid: None,
            auth_method: AuthMethod::None,
            password: "".try_into().unwrap(),
            channel: None,
        };
        let conf = Configuration::Mixed(sta_conf, ap_conf);
        let mut wifi = esp32_get_wifi()?.lock().await;
        wifi.set_configuration(&conf)?;
        wifi.start().await?;

        Ok(Self {
            storage,
            config: RefCell::new(conf),
        })
    }
    async fn scan_networks(&self) -> Result<Vec<AccessPointInfo>, EspError> {
        let mut wifi = esp32_get_wifi()?.lock().await;
        wifi.scan().await
    }
    async fn try_connect_to(&self, ssid: &str, password: &str) -> Result<(), EspError> {
        let mut wifi = esp32_get_wifi()?.lock().await;
        {
            let mut conf = self.config.borrow_mut();
            let (sta, _) = conf.as_mixed_conf_mut();
            log::info!("attempting to connect to {} {}", ssid, password);
            sta.ssid = ssid.try_into().unwrap();
            sta.auth_method = AuthMethod::None;
            sta.password = password.try_into().unwrap();
            wifi.set_configuration(&conf)?;
        }
        wifi.connect().await?;

        self.storage
            .store_wifi_credentials(&WifiCredentials {
                ssid: ssid.to_owned(),
                pwd: password.to_owned(),
            })
            .map_err(|_| EspError::from_non_zero(NonZeroI32::new(1).unwrap()))?;
        log::info!("connection successful");
        Ok(())
    }
}
