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
use std::{fmt::Debug, pin::Pin, time::Duration};

#[cfg(feature = "ota")]
use crate::common::{exec::Executor, ota};

pub struct ConfigMonitor<'a, Storage> {
    curr_config: Box<RobotConfig>, //config for robot gotten from last robot startup, aka inputted from entry
    storage: Storage,
    #[cfg(feature = "ota")]
    executor: Executor,
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
        #[cfg(feature = "ota")] executor: Executor,
        restart_hook: impl Fn() + 'a,
    ) -> Self {
        Self {
            curr_config,
            storage,
            #[cfg(feature = "ota")]
            executor,
            restart_hook: Box::new(restart_hook),
        }
    }

    fn restart(&self) -> ! {
        log::warn!("Robot configuration change detected restarting micro-rdk");
        (self.restart_hook)();
        unreachable!();
    }
}
impl<Storage> PeriodicAppClientTask for ConfigMonitor<'_, Storage>
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
                .as_ref()
                .config
                .as_ref()
                .is_some_and(|cfg| *cfg != *self.curr_config)
            {
                log::info!("new robot config detected");

                #[cfg(feature = "ota")]
                {
                    let config = new_config.config.clone().unwrap();
                    if let Some(service) = config
                        .services
                        .iter()
                        .find(|&service| service.model == *ota::OTA_MODEL_TRIPLET)
                    {
                        match ota::OtaService::from_config(
                            service,
                            self.storage.clone(),
                            self.executor.clone(),
                        ) {
                            Ok(mut ota) => {
                                let curr_metadata = ota.stored_metadata().await;
                                if curr_metadata.version != ota.pending_version {
                                    log::info!(
                                        "firmware is out of date, updating from `{}` to `{}`",
                                        curr_metadata.version,
                                        ota.pending_version
                                    );
                                    while let Err(ota_err) = ota.update().await {
                                        log::error!("failed to update firmware: {}", ota_err);
                                        log::info!("retrying firmware update");
                                    }
                                    log::info!("firmware update successful");
                                };
                                let curr_metadata = ota.stored_metadata().await;
                                log::info!(
                                    "firmware is up to date: version `{}`",
                                    curr_metadata.version
                                );
                            }
                            Err(e) => {
                                log::error!("failed to build ota service: {}", e.to_string());
                                log::error!("ota service config: {:?}", service);
                            }
                        };
                    }
                }

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
