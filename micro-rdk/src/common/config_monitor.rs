use super::app_client::{AppClient, AppClientError, PeriodicAppClientTask};
use crate::proto::app::v1::ConfigResponse;
use futures_lite::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct ConfigMonitor<'a> {
    restart_hook: Option<Box<dyn FnOnce() + 'a>>,
    curr_config: ConfigResponse, //config for robot gotten from last robot startup, aka inputted from entry
}

impl<'a> ConfigMonitor<'a> {
    pub fn new(restart_hook: impl FnOnce() + 'a, curr_config: ConfigResponse) -> Self {
        Self {
            restart_hook: Some(Box::new(restart_hook)),
            curr_config,
        }
    }

    fn restart(&mut self) -> ! {
        log::warn!("Robot configuration change detected");
        (self.restart_hook.take().unwrap())();
        unreachable!();
    }
}

impl<'a> PeriodicAppClientTask for ConfigMonitor<'a> {
    fn name(&self) -> &str {
        "ConfigMonitor"
    }

    fn get_default_period(&self) -> Duration {
        Duration::from_secs(10)
    }

    fn invoke<'c, 'b: 'c>(
        &'b mut self,
        app_client: &'c AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'c>> {
        Box::pin(async move {
            let (app_config, _cfg_received_datetime) = app_client.get_config(None).await.unwrap();
            match self.curr_config == *app_config {
                true => Ok(Some(self.get_default_period())),
                false => self.restart(),
            }
        })
    }
}
