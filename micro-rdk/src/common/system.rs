use std::fmt::Display;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

#[cfg(not(feature = "esp32"))]
use async_io::Timer;
use async_lock::Mutex as AsyncMutex;
use thiserror::Error;

use super::app_client::{AppClient, PeriodicAppClientTask};
use super::log::LogUploadTask;
use super::runtime::terminate;

// TODO: Find a way to do this without introducing mutable global state
static SHUTDOWN_EVENT: LazyLock<Arc<AsyncMutex<Option<SystemEvent>>>> =
    LazyLock::new(|| Arc::new(AsyncMutex::new(None)));

#[derive(Debug, Clone)]
pub enum SystemEvent {
    Restart,
    #[allow(dead_code)]
    DeepSleep(Option<Duration>),
}

#[derive(Debug, Error)]
pub enum SystemEventError {
    #[error("tried to request system event {0} but event {1} already requested")]
    SystemEventAlreadyRequested(SystemEvent, SystemEvent),
}

impl Display for SystemEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Restart => "restart".to_string(),
                Self::DeepSleep(dur) => dur.as_ref().map_or("enter deep sleep".to_string(), |d| {
                    format!("enter deep sleep for {} microseconds", d.as_micros())
                }),
            }
        )
    }
}

pub(crate) async fn send_system_event(event: SystemEvent) -> Result<(), SystemEventError> {
    log::info!("received call to {}", event);
    let mut current_event = SHUTDOWN_EVENT.lock().await;
    if current_event.is_none() {
        let _ = current_event.insert(event);
        Ok(())
    } else {
        let current_event_clone = (*current_event).clone().unwrap();
        Err(SystemEventError::SystemEventAlreadyRequested(
            event,
            current_event_clone,
        ))
    }
}

pub(crate) fn shutdown_requested() -> bool {
    SHUTDOWN_EVENT.lock_blocking().is_some()
}

pub(crate) async fn shutdown_requested_nonblocking() -> bool {
    SHUTDOWN_EVENT.lock().await.is_some()
}

pub(crate) async fn force_shutdown(app_client: Option<AppClient>) {
    // flush logs
    if let Some(app_client) = app_client.as_ref() {
        let log_task = LogUploadTask;
        let _ = log_task.invoke(app_client).await;
    }
    match *SHUTDOWN_EVENT.lock().await {
        Some(SystemEvent::Restart) => terminate(),
        Some(SystemEvent::DeepSleep(dur)) => {
            #[cfg(feature = "esp32")]
            {
                if let Some(dur) = dur {
                    let dur_micros = dur.as_micros() as u64;
                    let result: crate::esp32::esp_idf_svc::sys::esp_err_t;
                    unsafe {
                        result = crate::esp32::esp_idf_svc::sys::esp_sleep_enable_timer_wakeup(
                            dur_micros,
                        );
                    }
                    if result != crate::esp32::esp_idf_svc::sys::ESP_OK {
                        unreachable!("duration requested too long")
                    }
                    log::warn!("Esp32 entering deep sleep for {} microseconds!", dur_micros);
                }

                unsafe {
                    crate::esp32::esp_idf_svc::sys::esp_deep_sleep_start();
                }
            }
            #[cfg(not(feature = "esp32"))]
            if let Some(dur) = dur {
                log::warn!(
                    "Simulating deep sleep for {} microseconds!",
                    dur.as_micros()
                );
                async_io::block_on(Timer::after(dur));
                terminate();
            } else {
                log::error!("native builds do not support alternate wake up sources and can only sleep for a duration")
            }
        }
        None => {
            log::error!("call to shutdown/restart without request to system, terminating");
            terminate()
        }
    }
}
