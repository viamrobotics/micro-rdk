use std::{
    ffi::CString,
    fmt::Display,
    net::Ipv4Addr,
    ops::{Index, IndexMut},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
};

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

use esp_idf_svc::{
    hal::modem::WifiModem,
    sys::{
        esp_interface_t_ESP_IF_WIFI_STA, esp_wifi_get_config, esp_wifi_set_config, wifi_config_t,
        wifi_scan_method_t_WIFI_ALL_CHANNEL_SCAN, wifi_sort_method_t_WIFI_CONNECT_AP_BY_SIGNAL,
    },
    timer::EspTaskTimerService,
    wifi::AsyncWifi,
};
use futures_util::lock::Mutex;
use once_cell::sync::OnceCell;

use crate::{common::credentials_storage::WifiCredentials, esp32::esp_idf_svc::sys::EspError};

#[cfg(feature = "qemu")]
use crate::esp32::esp_idf_svc::eth::{BlockingEth, EspEth, OpenEth};

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
                .map_err(|_| NetworkError::HeaplessStringConversionError)?,
            auth_method: AuthMethod::None,
            password: wifi_creds
                .pwd
                .as_str()
                .try_into()
                .map_err(|_| NetworkError::HeaplessStringConversionError)?,
            ..Default::default()
        });
        let mut wifi = esp32_get_wifi()?.lock().await;

        crate::esp32::esp_idf_svc::sys::esp!(unsafe {
            esp_wifi_set_ps(crate::esp32::esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
        })?;

        wifi.stop().await?;
        wifi.set_configuration(&config)?;

        let mut sta_config = wifi_config_t::default();

        // Change the connection behavior to do a full scan and selecting the AP with the
        // strongest signal, instead of connecting to the first found AP which may not be the best
        // AP.
        match esp_idf_svc::sys::esp!(unsafe {
            esp_wifi_get_config(esp_interface_t_ESP_IF_WIFI_STA, &mut sta_config as *mut _)
        }) {
            Ok(_) => {
                sta_config.sta.scan_method = wifi_scan_method_t_WIFI_ALL_CHANNEL_SCAN;
                sta_config.sta.sort_method = wifi_sort_method_t_WIFI_CONNECT_AP_BY_SIGNAL;

                if let Err(e) = esp_idf_svc::sys::esp!(unsafe {
                    esp_wifi_set_config(esp_interface_t_ESP_IF_WIFI_STA, &mut sta_config as *mut _)
                }) {
                    log::warn!("couldn't update wifi station scan/sort config {:?}", e);
                }
            }
            Err(e) => {
                log::warn!("couldn't get wifi station config {:?}", e);
            }
        }

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

#[cfg(feature = "qemu")]
pub fn eth_configure<'d, T>(
    sl_stack: &EspSystemEventLoop,
    eth: EspEth<'d, T>,
) -> Result<Box<BlockingEth<EspEth<'d, T>>>, EspError> {
    let mut eth = BlockingEth::wrap(eth, sl_stack.clone())?;
    eth.start()?;
    eth.wait_netif_up()?;
    Ok(Box::new(eth))
}

#[cfg(feature = "qemu")]
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

enum ESP32NetifHandle {
    Esp32WifiSta,
    Esp32Eth,
}

impl IndexMut<ESP32NetifHandle> for [*mut esp_idf_svc::sys::esp_netif_t; 2] {
    fn index_mut(&mut self, index: ESP32NetifHandle) -> &mut Self::Output {
        match index {
            ESP32NetifHandle::Esp32WifiSta => &mut self[0],
            ESP32NetifHandle::Esp32Eth => &mut self[1],
        }
    }
}

impl Index<ESP32NetifHandle> for [*mut esp_idf_svc::sys::esp_netif_t; 2] {
    type Output = *mut esp_idf_svc::sys::esp_netif_t;
    fn index(&self, index: ESP32NetifHandle) -> &Self::Output {
        match index {
            ESP32NetifHandle::Esp32WifiSta => &self[0],
            ESP32NetifHandle::Esp32Eth => &self[1],
        }
    }
}

impl Display for ESP32NetifHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str = match self {
            ESP32NetifHandle::Esp32WifiSta => "WIFI_STA_DEF",
            ESP32NetifHandle::Esp32Eth => "ETH_DEF",
        };
        write!(f, "{}", as_str)
    }
}

struct Esp32NetifHelper {
    netif_hnds: [*mut esp_idf_svc::sys::esp_netif_t; 2],
}

impl Default for Esp32NetifHelper {
    fn default() -> Self {
        Self::new()
    }
}

impl Esp32NetifHelper {
    fn new() -> Self {
        let mut netif_hnds: [*mut esp_idf_svc::sys::esp_netif_t; 2] =
            [std::ptr::null_mut(), std::ptr::null_mut()];
        let wifi_key = CString::new(ESP32NetifHandle::Esp32WifiSta.to_string()).unwrap();

        netif_hnds[ESP32NetifHandle::Esp32WifiSta] =
            unsafe { esp_idf_svc::sys::esp_netif_get_handle_from_ifkey(wifi_key.as_ptr()) };

        let eth_key = CString::new(ESP32NetifHandle::Esp32Eth.to_string()).unwrap();
        netif_hnds[ESP32NetifHandle::Esp32Eth] =
            unsafe { esp_idf_svc::sys::esp_netif_get_handle_from_ifkey(eth_key.as_ptr()) };
        Self { netif_hnds }
    }
    fn get_ip_addr(&self) -> Result<u32, NetworkError> {
        let mut ip_info: esp_idf_svc::sys::esp_netif_ip_info_t = Default::default();
        if unsafe {
            esp_idf_svc::sys::esp!(esp_idf_svc::sys::esp_netif_get_ip_info(
                self.netif_hnds[ESP32NetifHandle::Esp32WifiSta],
                &mut ip_info as *mut _,
            ))
        }
        .is_ok()
        {
            return Ok(ip_info.ip.addr);
        }
        if unsafe {
            esp_idf_svc::sys::esp!(esp_idf_svc::sys::esp_netif_get_ip_info(
                self.netif_hnds[ESP32NetifHandle::Esp32Eth],
                &mut ip_info as *mut _,
            ))
        }
        .is_ok()
        {
            return Ok(ip_info.ip.addr);
        }

        Err(NetworkError::NoIpConfigured)
    }
}

#[derive(Clone)]
struct Esp32ExternallyManagerNetworkInner {
    connected: Arc<AtomicBool>,
    ipv4: Arc<AtomicU32>,
}

pub struct Esp32ExternallyManagedNetwork {
    inner: Box<Esp32ExternallyManagerNetworkInner>,
}

impl Network for Esp32ExternallyManagedNetwork {
    fn get_ip(&self) -> Ipv4Addr {
        let ip = self.inner.ipv4.load(Ordering::Acquire);
        Ipv4Addr::from(ip.to_be())
    }
    fn is_connected(&self) -> Result<bool, NetworkError> {
        Ok(self.inner.connected.load(Ordering::Acquire))
    }
}

impl Drop for Esp32ExternallyManagedNetwork {
    fn drop(&mut self) {
        unsafe {
            esp_idf_svc::sys::esp_event_handler_unregister(
                esp_idf_svc::sys::ETH_EVENT,
                esp_idf_svc::sys::ESP_EVENT_ANY_ID,
                Some(Self::callback),
            );
            esp_idf_svc::sys::esp_event_handler_unregister(
                esp_idf_svc::sys::WIFI_EVENT,
                esp_idf_svc::sys::ESP_EVENT_ANY_ID,
                Some(Self::callback),
            );
            esp_idf_svc::sys::esp_event_handler_unregister(
                esp_idf_svc::sys::IP_EVENT,
                esp_idf_svc::sys::ESP_EVENT_ANY_ID,
                Some(Self::callback),
            );
        };
    }
}

impl Default for Esp32ExternallyManagedNetwork {
    fn default() -> Self {
        Self::new()
    }
}

// Used when the esp32 network in managed by external code (like C)
impl Esp32ExternallyManagedNetwork {
    pub fn new() -> Self {
        // First we need to check if any netif interface is connected.
        // if connected we may not get an IP_EVENT allowing to set the ip for the Network interface
        let help = Esp32NetifHelper::default();
        // Default to ip 0
        let ip = help
            .get_ip_addr()
            .map_or(AtomicU32::default(), AtomicU32::new);
        // Assume not connected if no ip is acquired
        let connected = if ip.load(Ordering::Relaxed) != 0 {
            AtomicBool::new(true)
        } else {
            AtomicBool::new(false)
        };

        let data = Box::new(Esp32ExternallyManagerNetworkInner {
            connected: Arc::new(connected),
            ipv4: Arc::new(ip),
        });

        // Would be better to instantiate the EspSystemEventLoop but since the
        // default eventloop will likely be already created because we have an externally managed network we
        // won't be able to do that. (the call to EspSystemEventLoop::take will fail with ESP_ERR_INVALID_STATE)
        // This call does two checks,
        // 1) if the loop is not instantiated then create it
        // 2) confirm the loop is already instantiated (ESP_ERR_INVALID_STATE)
        // Any other errors are fatal
        let hnd = unsafe { esp_idf_svc::sys::esp_event_loop_create_default() };
        if hnd != 0 && hnd != esp_idf_svc::sys::ESP_ERR_INVALID_STATE {
            panic!("esp default event loop cannot be instantiated")
        }

        let mut this = Self { inner: data };

        if let Err(err) = unsafe {
            esp_idf_svc::sys::esp!(esp_idf_svc::sys::esp_event_handler_register(
                esp_idf_svc::sys::IP_EVENT,
                esp_idf_svc::sys::ESP_EVENT_ANY_ID,
                Some(Self::callback),
                this.inner.as_mut() as *mut Esp32ExternallyManagerNetworkInner as *mut _
            ))
        } {
            log::error!("failed to register IP_EVENT handler cause {:?}", err);
        }

        if let Err(err) = unsafe {
            esp_idf_svc::sys::esp!(esp_idf_svc::sys::esp_event_handler_register(
                esp_idf_svc::sys::WIFI_EVENT,
                esp_idf_svc::sys::ESP_EVENT_ANY_ID,
                Some(Self::callback),
                this.inner.as_mut() as *mut Esp32ExternallyManagerNetworkInner as *mut _
            ))
        } {
            log::error!("failed to register WIFI_EVENT handler cause {:?}", err);
        };

        if let Err(err) = unsafe {
            esp_idf_svc::sys::esp!(esp_idf_svc::sys::esp_event_handler_register(
                esp_idf_svc::sys::ETH_EVENT,
                esp_idf_svc::sys::ESP_EVENT_ANY_ID,
                Some(Self::callback),
                this.inner.as_mut() as *mut Esp32ExternallyManagerNetworkInner as *mut _
            ))
        } {
            log::error!("failed to register ETH_EVENT handler cause {:?}", err);
        };

        this
    }
    unsafe extern "C" fn callback(
        ev_hnd_arg: *mut std::ffi::c_void,
        ev_base: esp_idf_svc::sys::esp_event_base_t,
        ev_id: i32,
        ev_data: *mut std::ffi::c_void,
    ) {
        let data: &mut Esp32ExternallyManagerNetworkInner = &mut *(ev_hnd_arg as *mut _);
        let ev_id = ev_id as u32;
        if ev_base == esp_idf_svc::sys::IP_EVENT {
            // receiving an IP_EVENT is the only event that can transition us to connected state
            if ev_id == esp_idf_svc::sys::ip_event_t_IP_EVENT_STA_GOT_IP
                || ev_id == esp_idf_svc::sys::ip_event_t_IP_EVENT_ETH_GOT_IP
            {
                let ip_event: &mut esp_idf_svc::sys::ip_event_got_ip_t = &mut *(ev_data as *mut _);
                if ip_event.ip_changed {
                    data.ipv4.store(ip_event.ip_info.ip.addr, Ordering::Release);
                }
                data.connected.store(true, Ordering::Release);
            }
            if ev_id == esp_idf_svc::sys::ip_event_t_IP_EVENT_STA_LOST_IP
                || ev_id == esp_idf_svc::sys::ip_event_t_IP_EVENT_ETH_LOST_IP
            {
                data.connected.store(false, Ordering::Release);
            }
        }
        if ev_base == esp_idf_svc::sys::WIFI_EVENT
            && ev_id == esp_idf_svc::sys::wifi_event_t_WIFI_EVENT_STA_DISCONNECTED
        {
            data.connected.store(false, Ordering::Release);
        }

        if ev_base == esp_idf_svc::sys::ETH_EVENT
            && ev_id == esp_idf_svc::sys::eth_event_t_ETHERNET_EVENT_DISCONNECTED
        {
            data.connected.store(false, Ordering::Release);
        }
    }
}
