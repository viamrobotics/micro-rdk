use super::{
    app_client::{AppClient, AppClientError, PeriodicAppClientTask},
    conn::viam::ViamServerStorage,
};
use crate::{
    common::{
        config::AgentConfig, credentials_storage::RobotConfigurationStorage, grpc::ServerError,
    },
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
        log::warn!("machine configuration change detected - restarting micro-rdk");
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
            #[allow(unused_mut)]
            let mut reboot = false;
            let (new_config, _cfg_received_datetime) = app_client
                .get_app_config(None)
                .or(async {
                    let _ = Timer::after(Duration::from_secs(60)).await;
                    Err(AppClientError::AppClientRequestTimeout)
                })
                .await?;

            if let Some(config) = new_config.as_ref().config.as_ref() {
                #[cfg(feature = "ota")]
                {
                    if let Some(service) = config
                        .services
                        .iter()
                        .find(|&service| service.model == *ota::OTA_MODEL_TRIPLET)
                    {
                        // TODO(RSDK-9676): new OtaService created at every invocation, not ideal
                        match ota::OtaService::from_config(
                            service,
                            self.storage.clone(),
                            self.executor.clone(),
                        ) {
                            Ok(mut ota) => match ota.update().await {
                                Ok(needs_reboot) => reboot = needs_reboot,
                                Err(e) => log::error!("failed to complete ota update: {}", e),
                            },
                            Err(e) => log::error!(
                                "failed to create ota service from config:{} - {:?}",
                                e,
                                service,
                            ),
                        }
                    }
                }

                if *config != *self.curr_config {
                    if let Err(e) = self.storage.reset_robot_configuration() {
                        log::warn!(
                            "failed to reset machine config after new config detected: {}",
                            e
                        );
                    } else {
                        reboot = true;
                    }
                }
            }

            if let Ok(device_agent_config) = app_client.get_agent_config().await {
                if let Ok(agent_config) = AgentConfig::try_from(device_agent_config.as_ref()) {
                    log::debug!("agent config: {:?}", agent_config);

                    let stored = self
                        .storage
                        .get_network_settings()
                        .inspect_err(|e| log::warn!("failed to get networks from NVS: {}", e))
                        .unwrap_or_default();

                    if agent_config.network_settings.len() != stored.len()
                        || agent_config
                            .network_settings
                            .iter()
                            .any(|net| !stored.contains(net))
                    {
                        log::info!("new network settings found in config");
                        if let Err(e) = self
                            .storage
                            .store_network_settings(&agent_config.network_settings)
                        {
                            log::error!("failed to store network settings to nvs: {}", e);
                        } else {
                            log::info!("successfully stored networks to nvs");
                            reboot = true;
                        }
                    }
                }
            }

            if reboot {
                log::info!("rebooting from config monitor...");
                // TODO(RSDK-9464): flush logs to app.viam before restarting
                self.restart();
            }

            Ok(Some(self.get_default_period()))
        })
    }
}
