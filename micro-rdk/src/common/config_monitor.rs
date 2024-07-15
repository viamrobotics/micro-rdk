use super::app_client::{AppClient, AppClientError, PeriodicAppClientTask};
#[cfg(feature = "provisioning")]
use crate::common::{
    grpc::ServerError,
    provisioning::storage::{RobotConfigurationStorage, WifiCredentialStorage},
};
use crate::proto::app::v1::ConfigResponse;
use futures_lite::Future;
use std::fmt::Debug;
use std::pin::Pin;
use std::time::Duration;

pub struct ConfigMonitor<'a, S>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    restart_hook: Option<Box<dyn FnOnce() + 'a>>,
    curr_config: ConfigResponse, //config for robot gotten from last robot startup, aka inputted from entry
    storage: S,
}

impl<'a, S> ConfigMonitor<'a, S>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    pub fn new(restart_hook: impl FnOnce() + 'a, curr_config: ConfigResponse, storage: S) -> Self {
        Self {
            restart_hook: Some(Box::new(restart_hook)),
            curr_config,
            storage,
        }
    }

    fn restart(&mut self) -> ! {
        log::warn!("Robot configuration change detected restarting micro-rdk");
        (self.restart_hook.take().unwrap())();
        unreachable!();
    }
}
impl<'a, S> PeriodicAppClientTask for ConfigMonitor<'a, S>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    fn name(&self) -> &str {
        "ConfigMonitor"
    }

    fn get_default_period(&self) -> Duration {
        Duration::from_secs(10)
    }

    // TODO(RSDK-8160): Update "restart on config" to compare config version instead of deep
    // comparison of config response, which relies on RSDK-8023 adding config version
    fn invoke<'c, 'b: 'c>(
        &'b mut self,
        app_client: &'c AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'c>> {
        Box::pin(async move {
            let (_app_client_config, new_config, _cfg_received_datetime) =
                app_client.clone().get_config(None).await.unwrap();
            match self.curr_config == *new_config {
                true => Ok(Some(self.get_default_period())),
                false => {
                    if let Err(e) = self
                        .storage
                        .store_robot_configuration(new_config.config.unwrap())
                    {
                        log::warn!("Failed to store new robot configuration from app: {}", e);
                    }
                    self.restart();
                }
            }
        })
    }
}
