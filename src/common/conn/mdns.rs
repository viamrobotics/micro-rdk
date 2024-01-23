use thiserror::Error;

#[derive(Error, Debug)]
pub enum MdnsError {
    #[error("couldn't add mdns")]
    MdnsAddServiceError(String),
    #[error("couldn't init mdns")]
    MdnsInitServiceError(String),
}

pub struct NoMdns;

impl Mdns for NoMdns {
    fn add_service(
        &mut self,
        _: &str,
        _: impl AsRef<str>,
        _: impl AsRef<str>,
        _: u16,
        _: &[(&str, &str)],
    ) -> Result<(), MdnsError> {
        Ok(())
    }
    fn set_hostname(&mut self, _: &str) -> Result<(), MdnsError> {
        Ok(())
    }
}

pub trait Mdns {
    fn add_service(
        &mut self,
        _: &str,
        _: impl AsRef<str>,
        _: impl AsRef<str>,
        _: u16,
        _: &[(&str, &str)],
    ) -> Result<(), MdnsError>;

    fn set_hostname(&mut self, _: &str) -> Result<(), MdnsError> {
        Ok(())
    }
}
