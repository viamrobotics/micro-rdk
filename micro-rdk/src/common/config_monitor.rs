use super::{
    app_client::{AppClient, AppClientError, PeriodicAppClientTask},
    conn::viam::ViamServerStorage,
};
use crate::{
    common::{credentials_storage::RobotConfigurationStorage, grpc::ServerError},
    proto::app::v1::RobotConfig,
};
use async_io::Timer;
use futures_lite::{Future, FutureExt};
use std::fmt::Debug;
use std::pin::Pin;
use std::time::Duration;

pub struct ConfigMonitor<'a, Storage> {
    curr_config: Box<RobotConfig>, //config for robot gotten from last robot startup, aka inputted from entry
    storage: Storage,
    restart_hook: Box<dyn Fn() + 'a>,
}

impl<'a, Storage> ConfigMonitor<'a, Storage>
where
    Storage: ViamServerStorage,
    <Storage as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<Storage as RobotConfigurationStorage>::Error>,
{
    pub fn new(
        curr_config: Box<RobotConfig>,
        storage: Storage,
        restart_hook: impl Fn() + 'a,
    ) -> Self {
        Self {
            curr_config,
            storage,
            restart_hook: Box::new(restart_hook),
        }
    }

    fn restart(&self) -> ! {
        log::warn!("Robot configuration change detected restarting micro-rdk");
        (self.restart_hook)();
        unreachable!();
    }
}
impl<'a, Storage> PeriodicAppClientTask for ConfigMonitor<'a, Storage>
where
    Storage: ViamServerStorage,
    <Storage as RobotConfigurationStorage>::Error: Debug,
    ServerError: From<<Storage as RobotConfigurationStorage>::Error>,
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
            let (new_config, _cfg_received_datetime) = app_client
                .get_app_config(None)
                .or(async {
                    let _ = Timer::after(Duration::from_secs(60)).await;
                    Err(AppClientError::AppClientRequestTimeout)
                })
                .await?;

            if new_config
                .config
                .is_some_and(|cfg| cfg != *self.curr_config)
            {
                if let Err(e) = self.storage.reset_robot_configuration() {
                    log::warn!(
                        "Failed to reset robot config after new config detected: {}",
                        e
                    );
                } else {
                    self.restart();
                }
            }

            Ok(Some(self.get_default_period()))
        })
    }
}
