use super::app_client::{AppClient, AppClientError, PeriodicAppClientTask};
use futures_lite::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct RestartMonitor<'a> {
    restart_hook: Option<Box<dyn FnOnce() + 'a>>,
}

impl<'a> RestartMonitor<'a> {
    pub fn new(restart_hook: impl FnOnce() + 'a) -> Self {
        Self {
            restart_hook: Some(Box::new(restart_hook)),
        }
    }

    fn restart(&mut self) -> ! {
        log::warn!("Restart request received - restarting or terminating now...");
        (self.restart_hook.take().unwrap())();
        unreachable!();
    }
}

impl<'a> PeriodicAppClientTask for RestartMonitor<'a> {
    fn name(&self) -> &str {
        "RestartMonitor"
    }

    fn get_default_period(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn invoke<'c, 'b: 'c>(
        &'b mut self,
        app_client: &'c AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'c>> {
        Box::pin(async move {
            match app_client.check_for_restart().await {
                Ok(None) => self.restart(),
                other => other,
            }
        })
    }
}
