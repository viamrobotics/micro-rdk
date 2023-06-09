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
        let ty_domain = format!("{}.{}.local.", service_type.as_ref(), proto.as_ref());

        let props: HashMap<String, String> = txt
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect();

        let service = ServiceInfo::new(
            &ty_domain,
            instance_name,
            &self.hostname,
            self.ip,
            port,
            Some(props),
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
