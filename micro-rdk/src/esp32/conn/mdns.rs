// TODO(RSDK-8993): Obtain this from the esp-idf component registry so
// we can upgrade `esp-idf-svc`.
use crate::esp32::esp_idf_svc::mdns::EspMdns;

use crate::common::conn::mdns::{Mdns, MdnsError};

pub struct Esp32Mdns {
    inner: EspMdns,
    hostname: String,
}

impl Esp32Mdns {
    pub fn new(hostname: String) -> Result<Self, MdnsError> {
        Ok(Self {
            inner: EspMdns::take().map_err(|e| MdnsError::MdnsInitServiceError(e.to_string()))?,
            hostname,
        })
    }
    fn add_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        protocol: impl AsRef<str>,
        port: u16,
        txt: &[(&str, &str)],
    ) -> Result<(), MdnsError> {
        self.inner
            .set_hostname(self.hostname.clone())
            .map_err(|e| MdnsError::MdnsAddServiceError(e.to_string()))?;
        self.inner
            .add_service(Some(instance_name), service_type, protocol, port, txt)
            .map_err(|e| MdnsError::MdnsAddServiceError(e.to_string()))
    }
    fn remove_service(
        &mut self,
        _: &str,
        service_type: impl AsRef<str>,
        protocol: impl AsRef<str>,
    ) -> Result<(), MdnsError> {
        self.inner
            .remove_service(service_type, protocol)
            .map_err(|e| MdnsError::MdnsRemoveServiceError(e.to_string()))
    }
    fn set_hostname(&mut self, hostname: &str) -> Result<(), MdnsError> {
        self.hostname = hostname.to_owned();
        Ok(())
    }
}

impl Mdns for Esp32Mdns {
    fn add_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        protocol: impl AsRef<str>,
        port: u16,
        txt: &[(&str, &str)],
    ) -> Result<(), MdnsError> {
        self.add_service(instance_name, service_type, protocol, port, txt)
    }
    fn set_hostname(&mut self, hostname: &str) -> Result<(), MdnsError> {
        self.set_hostname(hostname)
    }
    fn remove_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        protocol: impl AsRef<str>,
    ) -> Result<(), MdnsError> {
        self.remove_service(instance_name, service_type, protocol)
    }
}

impl Mdns for &mut Esp32Mdns {
    fn add_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        protocol: impl AsRef<str>,
        port: u16,
        txt: &[(&str, &str)],
    ) -> Result<(), MdnsError> {
        (*self).add_service(instance_name, service_type, protocol, port, txt)
    }
    fn set_hostname(&mut self, hostname: &str) -> Result<(), MdnsError> {
        (*self).set_hostname(hostname)
    }
    fn remove_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        protocol: impl AsRef<str>,
    ) -> Result<(), MdnsError> {
        (*self).remove_service(instance_name, service_type, protocol)
    }
}
