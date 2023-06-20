use thiserror::Error;

#[derive(Error, Debug)]
pub enum MdnsError {
    #[error("couldn't add mdns")]
    MdnsAddServiceError(String),
    #[error("couldn't init mdns")]
    MdnsInitServiceError(String),
}

pub trait Mdns {
    fn add_service(
        &mut self,
        instance_name: &str,
        service_type: impl AsRef<str>,
        proto: impl AsRef<str>,
        port: u16,
        txt: &[(&str, &str)],
    ) -> Result<(), MdnsError>;

    fn set_hostname(&mut self, _: &str) -> Result<(), MdnsError> {
        Ok(())
    }
}
