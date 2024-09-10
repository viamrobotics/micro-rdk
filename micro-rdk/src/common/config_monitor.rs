use super::app_client::{AppClient, AppClientError, PeriodicAppClientTask};
use crate::common::{
    credentials_storage::{RobotConfigurationStorage, WifiCredentialStorage},
    grpc::ServerError,
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
    curr_config: ConfigResponse, //config for robot gotten from last robot startup, aka inputted from entry
    storage: S,
    restart_hook: Option<Box<dyn Fn() + 'a>>,
}

impl<'a, S> ConfigMonitor<'a, S>
where
    S: RobotConfigurationStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<S as RobotConfigurationStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
{
    pub fn new(curr_config: ConfigResponse, storage: S, restart_hook: impl Fn() + 'a) -> Self {
        Self {
            curr_config,
            storage,
            restart_hook: Some(Box::new(restart_hook)),
        }
    }

    fn restart(&self) -> ! {
        log::warn!("Robot configuration change detected restarting micro-rdk");
        (self.restart_hook.as_ref().unwrap())();
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
        &'b self,
        app_client: &'c AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'c>> {
        Box::pin(async move {
            if let Ok((new_config, _cfg_received_datetime)) = app_client.get_app_config(None).await
            {
                if self.curr_config != *new_config {
                    if let Err(e) = self.storage.reset_robot_configuration() {
                        log::warn!(
                            "Failed to reset robot config after new config detected: {}",
                            e
                        );
                    } else {
                        self.restart();
                    }
                }
            }
            Ok(Some(self.get_default_period()))
        })
    }
}
