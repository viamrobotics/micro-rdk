#![allow(dead_code)]
use std::{convert::Infallible, rc::Rc, sync::Mutex};

use crate::proto::provisioning::v1::CloudConfig;

#[derive(Clone, Default)]
pub struct RobotCredentials {
    robot_secret: String,
    robot_id: String,
}

impl RobotCredentials {
    pub(crate) fn robot_secret(&self) -> &str {
        &self.robot_secret
    }
    pub(crate) fn robot_id(&self) -> &str {
        &self.robot_id
    }
}

impl From<CloudConfig> for RobotCredentials {
    fn from(value: CloudConfig) -> Self {
        // TODO: make ticket : ignore app_address for now but need to add it later
        Self {
            robot_id: value.id,
            robot_secret: value.secret,
        }
    }
}

pub trait CredentialStorage {
    type Error;
    fn has_stored_credentials(&self) -> bool;
    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error>;
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error>;
}

/// Simple CrendentialStorage made for testing purposes
#[derive(Default, Clone)]
pub(crate) struct MemoryCredentialStorage {
    config: Rc<Mutex<Option<RobotCredentials>>>,
}

impl CredentialStorage for MemoryCredentialStorage {
    type Error = Infallible;
    fn has_stored_credentials(&self) -> bool {
        self.config.lock().unwrap().is_some()
    }
    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error> {
        let creds: RobotCredentials = cfg.into();
        let _ = self.config.lock().unwrap().insert(creds);
        Ok(())
    }
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error> {
        Ok(self
            .config
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_default()
            .clone())
    }
}
