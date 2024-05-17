#![allow(dead_code)]
use std::{collections::HashMap, net::Ipv4Addr};

use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::common::conn::mdns::{Mdns, MdnsError};

pub struct NativeMdns {
    inner: ServiceDaemon,
    hostname: String,
    ip: Ipv4Addr,
}

impl NativeMdns {
    pub fn new(hostname: String, ip: Ipv4Addr) -> Result<Self, MdnsError> {
        Ok(Self {
            inner: ServiceDaemon::new()
                .map_err(|e| MdnsError::MdnsInitServiceError(e.to_string()))?,
            hostname,
            ip,
        })
    }
    pub(crate) fn daemon(&self) -> ServiceDaemon {
        self.inner.clone()
    }
    fn add_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        proto: impl AsRef<str>,
        port: u16,
        txt: &[(&str, &str)],
    ) -> Result<(), MdnsError> {
        let ty_domain = format!("{}.{}.local.", service_type.as_ref(), proto.as_ref());
        let srv_hostname = format!("{}.{}", self.hostname, &ty_domain);

        let props: HashMap<String, String> = txt
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect();

        let service = ServiceInfo::new(
            &ty_domain,
            instance_name,
            &srv_hostname,
            format!("{}", self.ip),
            port,
            props,
        )
        .map_err(|e| MdnsError::MdnsAddServiceError(e.to_string()))?;

        self.inner
            .register(service)
            .map_err(|e| MdnsError::MdnsAddServiceError(e.to_string()))?;

        Ok(())
    }
    fn set_hostname(&mut self, hostname: &str) -> Result<(), MdnsError> {
        self.hostname = hostname.to_owned();
        Ok(())
    }
}

impl Drop for NativeMdns {
    fn drop(&mut self) {
        let _ = self.daemon().shutdown();
    }
}
impl Mdns for NativeMdns {
    fn add_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        proto: impl AsRef<str>,
        port: u16,
        txt: &[(&str, &str)],
    ) -> Result<(), MdnsError> {
        self.add_service(instance_name, service_type, proto, port, txt)
    }
    fn set_hostname(&mut self, hostname: &str) -> Result<(), MdnsError> {
        self.set_hostname(hostname)
    }
}

impl Mdns for &mut NativeMdns {
    fn add_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        proto: impl AsRef<str>,
        port: u16,
        txt: &[(&str, &str)],
    ) -> Result<(), MdnsError> {
        (*self).add_service(instance_name, service_type, proto, port, txt)
    }
    fn set_hostname(&mut self, hostname: &str) -> Result<(), MdnsError> {
        (*self).set_hostname(hostname)
    }
}
