use super::app_client::{AppClient, AppClientError, PeriodicAppClientTask};
use futures_lite::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct RestartMonitor {
    restart_hook: Option<Box<dyn FnOnce()>>,
}

impl RestartMonitor {
    pub fn new(restart_hook: impl FnOnce() + 'static) -> Self {
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

impl PeriodicAppClientTask for RestartMonitor {
    fn name(&self) -> &str {
        "RestartMonitor"
    }

    fn get_default_period(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn invoke<'b, 'a: 'b>(
        &'a mut self,
        app_client: &'b mut AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'b>> {
        Box::pin(async move {
            match app_client.check_for_restart().await {
                Ok(None) => {
                    self.restart();
                }
                Ok(Some(duration)) => Ok(Some(duration)),
                Err(e) => Err(e),
            }
        })
    }
}
