use std::net::Ipv4Addr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("error connecting to network")]
    ConnectionError,
    #[cfg(feature = "esp32")]
    #[error(transparent)]
    Esp32ConnectionError(#[from] crate::esp32::esp_idf_svc::sys::EspError),
    #[error("couldn't convert to heapless string")]
    HeapLessStringConversionError,
}

/// Reflects the representation of a network's status.
pub trait Network {
    /// Get the current IP address of the network interface.
    fn get_ip(&self) -> Ipv4Addr;

    /// Returns whether the underlying network interface is connected, *not* if
    /// internet access is available
    fn is_connected(&self) -> Result<bool, NetworkError>;
}

/// For networks managed outside of micro-rdk (for example, using micro-rdk as an ESP-IDF
/// component in a separate project), this struct is meant to simply communicate the IP
/// address statically. It will trivially always appear as connected because connectivity
/// management is external
#[repr(C)]
pub struct ExternallyManagedNetwork {
    ip: Ipv4Addr,
}

impl ExternallyManagedNetwork {
    pub fn new(ip: Ipv4Addr) -> Self {
        Self { ip }
    }
}

impl Network for ExternallyManagedNetwork {
    // TODO: provide a way for an external managed network to communicate a change in IP
    // address
    fn get_ip(&self) -> Ipv4Addr {
        self.ip
    }
    fn is_connected(&self) -> Result<bool, NetworkError> {
        Ok(true)
    }
}
