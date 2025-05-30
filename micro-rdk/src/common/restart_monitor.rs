use super::app_client::{AppClient, AppClientError, PeriodicAppClientTask};
use super::system::{send_system_event, SystemEvent};
use futures_lite::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct RestartMonitor;

impl PeriodicAppClientTask for RestartMonitor {
    fn name(&self) -> &str {
        "RestartMonitor"
    }

    fn get_default_period(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn invoke<'b, 'a: 'b>(
        &'a self,
        app_client: &'b AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'b>> {
        Box::pin(async move {
            match app_client.check_for_restart().await {
                Ok(None) => {
                    if let Err(err) = send_system_event(SystemEvent::Restart, false).await {
                        log::warn!("skipping action from restart monitor: {:?}", err);
                    };
                    Ok(None)
                }
                other => other,
            }
        })
    }
}
