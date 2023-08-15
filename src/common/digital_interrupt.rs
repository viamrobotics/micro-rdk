use super::config::{AttributeError, Kind};

#[derive(Copy, Clone, Debug)]
pub struct DigitalInterruptConfig {
    pub pin: i32,
}

impl TryFrom<Kind> for DigitalInterruptConfig {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        if !value.contains_key("pin")? {
            return Err(AttributeError::KeyNotFound("pin".to_string()));
        }
        let pin = value.get("pin")?.unwrap().try_into()?;
        Ok(DigitalInterruptConfig { pin })
    }
}

impl TryFrom<&Kind> for DigitalInterruptConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if !value.contains_key("pin")? {
            return Err(AttributeError::KeyNotFound("pin".to_string()));
        }
        let pin = value.get("pin")?.unwrap().try_into()?;
        Ok(DigitalInterruptConfig { pin })
    }
}
