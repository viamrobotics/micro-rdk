use std::convert::Infallible;

use crate::proto::provisioning::v1::CloudConfig;

pub trait Storage {
    type Error;
    fn has_stored_credentials(&self) -> bool;
    fn store_robot_credentials(&mut self, cfg: CloudConfig) -> Result<(), Self::Error>;
    fn get_robot_credentials(&self) -> Result<(String, String), Self::Error>;
}

#[derive(Default, Clone)]
pub(crate) struct MemoryCredentialStorage {
    robot_secret: Option<String>,
    robot_id: Option<String>,
}

impl Storage for MemoryCredentialStorage {
    type Error = Infallible;
    fn has_stored_credentials(&self) -> bool {
        self.robot_id.is_some() && self.robot_secret.is_some()
    }
    fn store_robot_credentials(&mut self, cfg: CloudConfig) -> Result<(), Self::Error> {
        // TODO: make ticket : ignore app_address for now but need to add it later
        self.robot_id = Some(cfg.id);
        self.robot_secret = Some(cfg.secret);
        Ok(())
    }
    fn get_robot_credentials(&self) -> Result<(String, String), Self::Error> {
        Ok((
            self.robot_id.as_ref().unwrap_or(&"".to_owned()).clone(),
            self.robot_secret.as_ref().unwrap_or(&"".to_owned()).clone(),
        ))
    }
}
