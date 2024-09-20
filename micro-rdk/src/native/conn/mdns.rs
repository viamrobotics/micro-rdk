#![allow(dead_code)]
use std::{collections::HashMap, net::Ipv4Addr, time::Duration};

use mdns_sd::{ServiceDaemon, ServiceInfo, UnregisterStatus};

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
        protocol: impl AsRef<str>,
        port: u16,
        txt: &[(&str, &str)],
    ) -> Result<(), MdnsError> {
        let ty_domain = format!("{}.{}.local.", service_type.as_ref(), protocol.as_ref());
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
    fn remove_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        protocol: impl AsRef<str>,
    ) -> Result<(), MdnsError> {
        let ty_domain = format!("{}.{}.local.", service_type.as_ref(), protocol.as_ref());
        let fullname = format!("{}.{}", instance_name, ty_domain);

        let recv = self
            .inner
            .unregister(&fullname)
            .map_err(|e| MdnsError::MdnsRemoveServiceError(e.to_string()))?;
        let ret = recv
            .recv_timeout(Duration::from_millis(300))
            .map_err(|_| MdnsError::MdnsRemoveServiceError("timeout".to_string()))?;
        match ret {
            UnregisterStatus::OK => Ok(()),
            UnregisterStatus::NotFound => {
                Err(MdnsError::MdnsRemoveServiceError("not found".to_owned()))
            }
        }
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

impl Mdns for &mut NativeMdns {
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
