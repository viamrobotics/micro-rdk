use std::{convert::Infallible, rc::Rc, sync::Mutex};

use crate::proto::provisioning::v1::CloudConfig;

pub trait Storage {
    type Error;
    fn has_stored_credentials(&self) -> bool;
    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error>;
    fn get_robot_credentials(&self) -> Result<(String, String), Self::Error>;
}

#[derive(Default, Clone)]
pub(crate) struct MemoryCredentialStorage {
    robot_secret: Rc<Mutex<Option<String>>>,
    robot_id: Rc<Mutex<Option<String>>>,
}

impl Storage for MemoryCredentialStorage {
    type Error = Infallible;
    fn has_stored_credentials(&self) -> bool {
        self.robot_id.lock().unwrap().is_some() && self.robot_secret.lock().unwrap().is_some()
    }
    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error> {
        // TODO: make ticket : ignore app_address for now but need to add it later
        let _ = self.robot_id.lock().unwrap().insert(cfg.id);
        let _ = self.robot_secret.lock().unwrap().insert(cfg.secret);
        Ok(())
    }
    fn get_robot_credentials(&self) -> Result<(String, String), Self::Error> {
        Ok((
            self.robot_id
                .lock()
                .unwrap()
                .as_ref()
                .unwrap_or(&"".to_owned())
                .clone(),
            self.robot_secret
                .lock()
                .unwrap()
                .as_ref()
                .unwrap_or(&"".to_owned())
                .clone(),
        ))
    }
}
