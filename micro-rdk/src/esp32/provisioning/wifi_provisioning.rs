#![allow(dead_code)]
use esp_idf_svc::{
    handle::RawHandle,
    netif::EspNetif,
    sys::{self, esp_ip4_addr, ip4_addr, EspError},
    wifi::{
        AccessPointConfiguration, AccessPointInfo, AuthMethod, ClientConfiguration, Configuration,
        Protocol,
    },
};

use std::{cell::RefCell, ffi::c_void, net::Ipv4Addr};
use thiserror::Error;

use crate::{
    common::provisioning::{
        server::{NetworkInfo, WifiManager},
        storage::WifiCredentialStorage,
    },
    esp32::conn::network::esp32_get_wifi,
};

pub struct Esp32WifiProvisioningBuilder {
    ap_ip_addr: Ipv4Addr,
    ssid: String,
    password: String,
}

impl Default for Esp32WifiProvisioningBuilder {
    fn default() -> Self {
        let mut mac_address = [0_u8; 8];
        unsafe {
            sys::esp!(sys::esp_efuse_mac_get_default(mac_address.as_mut_ptr())).unwrap();
        };
        Self {
            ssid: format!(
                "esp32-micrordk-{:02X}{:02X}",
                mac_address[4], mac_address[5]
            ),
            password: "viamsetup".to_owned(),
            ap_ip_addr: Ipv4Addr::new(10, 42, 0, 1),
        }
    }
}

impl Esp32WifiProvisioningBuilder {
    pub fn set_ap_ip(mut self, ip: Ipv4Addr) -> Self {
        self.ap_ip_addr = ip;
        self
    }
    pub fn set_ap_ssid(mut self, ssid: String) -> Self {
        self.ssid = ssid;
        self
    }
    pub fn set_ap_password(mut self, password: String) -> Self {
        self.password = password;
        self
    }
    pub async fn build<S>(
        self,
        storage: S,
    ) -> Result<Esp32WifiProvisioning<S>, Esp32WifiProvisioningError>
    where
        S: WifiCredentialStorage,
        <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
    {
        Esp32WifiProvisioning::new(storage, &self.ssid, &self.password, self.ap_ip_addr).await
    }
}
#[derive(Error, Debug)]
pub enum Esp32WifiProvisioningError {
    #[error("cannot assign to heapless string")]
    HeaplessStringError,
    #[error(transparent)]
    EspError(#[from] EspError),
    #[error(transparent)]
    OtherError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub struct Esp32WifiProvisioning<S> {
    storage: S,
    esp_wifi_config: RefCell<Configuration>,
    ap_ip_addr: Ipv4Addr,
}

impl From<&AccessPointInfo> for NetworkInfo {
    fn from(value: &AccessPointInfo) -> Self {
        let mut info = NetworkInfo::default();
        info.0.ssid = value.ssid.to_string();
        info.0.security = value
            .auth_method
            .map_or("none".to_owned(), |auth| auth.to_string());
        info.0.signal = (2 * (value.signal_strength as i32 + 100)).clamp(0, 100);
        info.0.r#type = "2.4GhZ".to_owned();
        info
    }
}

impl<S> WifiManager for Esp32WifiProvisioning<S>
where
    S: WifiCredentialStorage,
    <S as WifiCredentialStorage>::Error: 'static + Send + Sync,
{
    type Error = Esp32WifiProvisioningError;
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
    fn get_ap_ip(&self) -> Ipv4Addr {
        self.ap_ip_addr
    }
}

impl<S> Esp32WifiProvisioning<S>
where
    S: WifiCredentialStorage,
    <S as WifiCredentialStorage>::Error: Send + Sync + 'static,
{
    async fn new(
        storage: S,
        ssid: &str,
        password: &str,
        ip: Ipv4Addr,
    ) -> Result<Self, Esp32WifiProvisioningError> {
        let ap_conf = AccessPointConfiguration {
            ssid: ssid
                .try_into()
                .map_err(|_| Esp32WifiProvisioningError::HeaplessStringError)?,
            ssid_hidden: false,
            channel: 10,
            secondary_channel: None,
            protocols: Protocol::P802D11B | Protocol::P802D11BG | Protocol::P802D11BGN,
            auth_method: esp_idf_svc::wifi::AuthMethod::WPA2Personal,
            password: password
                .try_into()
                .map_err(|_| Esp32WifiProvisioningError::HeaplessStringError)?,
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

        let this = Self {
            storage,
            esp_wifi_config: RefCell::new(conf),
            ap_ip_addr: ip,
        };

        this.set_ap_ip_base_address(ip, wifi.wifi_mut().ap_netif_mut())?;

        wifi.start().await?;

        Ok(this)
    }
    fn set_ap_ip_base_address(
        &self,
        addr: Ipv4Addr,
        netif: &mut EspNetif,
    ) -> Result<(), Esp32WifiProvisioningError> {
        let handle = netif.handle();
        let ip = esp_ip4_addr {
            addr: u32::from_le_bytes(addr.octets()),
        };
        let netmask = esp_ip4_addr {
            addr: u32::from_le_bytes([255, 255, 255, 0]),
        };
        let ip_info = sys::esp_netif_ip_info_t {
            ip,
            gw: ip,
            netmask,
        };

        let mut octet = addr.octets();
        octet[3] += 1;

        let start_ip = ip4_addr {
            addr: u32::from_le_bytes(octet),
        };
        octet[3] += 2;
        let end_ip = ip4_addr {
            addr: u32::from_le_bytes(octet),
        };

        let mut dhcps_leases = sys::dhcps_lease_t {
            enable: true,
            start_ip,
            end_ip,
        };

        unsafe { sys::esp!(sys::esp_netif_dhcps_stop(handle)) }?;
        unsafe { sys::esp!(sys::esp_netif_set_ip_info(handle, &ip_info as *const _)) }?;
        unsafe {
            sys::esp!(sys::esp_netif_dhcps_option(
                handle,
                sys::esp_netif_dhcp_option_mode_t_ESP_NETIF_OP_SET,
                sys::esp_netif_dhcp_option_id_t_ESP_NETIF_REQUESTED_IP_ADDRESS,
                &mut dhcps_leases as *mut _ as *mut c_void,
                std::mem::size_of::<sys::dhcps_lease_t>() as u32
            ))
        }?;

        let mut dns_config = sys::esp_netif_dns_info_t {
            ip: sys::esp_ip_addr_t {
                u_addr: sys::_ip_addr__bindgen_ty_1 { ip4: ip },
                type_: 0, // Ipv4Type
            },
        };

        unsafe {
            sys::esp!(sys::esp_netif_set_dns_info(
                handle,
                sys::esp_netif_dns_type_t_ESP_NETIF_DNS_MAIN,
                &mut dns_config as *mut _
            ))
        }?;

        unsafe { sys::esp!(sys::esp_netif_dhcps_start(handle)) }?;
        Ok(())
    }
    async fn scan_networks(&self) -> Result<Vec<AccessPointInfo>, Esp32WifiProvisioningError> {
        let mut wifi = esp32_get_wifi()?.lock().await;
        wifi.scan().await.map_err(Into::into)
    }
    async fn try_connect_to(
        &self,
        ssid: &str,
        password: &str,
    ) -> Result<(), Esp32WifiProvisioningError> {
        let mut wifi = esp32_get_wifi()?.lock().await;
        {
            let mut conf = self.esp_wifi_config.borrow_mut();
            let (sta, _) = conf.as_mixed_conf_mut();
            sta.ssid = ssid
                .try_into()
                .map_err(|_| Esp32WifiProvisioningError::HeaplessStringError)?;
            sta.auth_method = AuthMethod::None;
            sta.password = password
                .try_into()
                .map_err(|_| Esp32WifiProvisioningError::HeaplessStringError)?;
            wifi.set_configuration(&conf)?;
        }
        wifi.connect().await?;

        log::info!("connection successful");
        Ok(())
    }
}
