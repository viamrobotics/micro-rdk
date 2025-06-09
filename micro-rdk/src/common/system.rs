use std::{
    fmt::Display,
    sync::{Arc, LazyLock},
    time::Duration,
};

use super::{
    app_client::{AppClient, PeriodicAppClientTask},
    log::LogUploadTask,
    runtime::terminate,
};

#[cfg(feature = "esp32")]
use crate::esp32::esp_idf_svc::sys;
#[cfg(not(feature = "esp32"))]
use async_io::Timer;

use async_lock::Mutex as AsyncMutex;
use thiserror::Error;

#[derive(Default, Debug)]
pub enum FirmwareMode {
    #[default]
    Normal,
    DeepSleepBetweenDataSyncs,
}

impl Display for FirmwareMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(
            match self {
                Self::Normal => "normal",
                Self::DeepSleepBetweenDataSyncs => "deep sleep between data syncs",
            },
            f,
        )
    }
}

impl From<Option<&str>> for FirmwareMode {
    fn from(value: Option<&str>) -> Self {
        match value {
            Some("deep_sleep_between_data_syncs") => Self::DeepSleepBetweenDataSyncs,
            Some("normal") => Self::Normal,
            _ => Self::Normal,
        }
    }
}

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
    #[error("{0}")]
    EnableUlpWakeupError(String),
    #[error("system does not support ULP wakeup")]
    UlpWakeupNotSupported,
}

impl Display for SystemEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Restart => "restart".to_string(),
                Self::DeepSleep(duration) => {
                    duration
                        .as_ref()
                        .map_or("enter deep sleep".to_string(), |d| {
                            format!("enter deep sleep for {} microseconds", d.as_micros())
                        })
                }
            }
        )
    }
}

pub(crate) async fn send_system_event(
    event: SystemEvent,
    force: bool,
) -> Result<(), SystemEventError> {
    let mut current_event = SHUTDOWN_EVENT.lock().await;
    if current_event.is_none() || force {
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

pub(crate) fn enable_ulp_wakeup() -> Result<(), SystemEventError> {
    #[cfg(feature = "esp32")]
    {
        let result = unsafe { sys::esp_sleep_enable_ulp_wakeup() };

        match result {
            sys::ESP_OK => Ok(()),
            sys::ESP_ERR_NOT_SUPPORTED => Err(SystemEventError::EnableUlpWakeupError(format!(
                "additional current to external 32kHz crystal is enabled, cannot enable ULP wakeup source: {:?}",
                result

            ))),
            sys::ESP_ERR_INVALID_STATE => Err(SystemEventError::EnableUlpWakeupError(format!(
                "co-processor not enabled or wakeup trigger conflicts with ulp wakeup: {:?}",
                result
            ))),
            _ => Err(SystemEventError::EnableUlpWakeupError(format!(
                "failed to enable ULP as wakeup source: {:?}",
                result
            ))),
        }
    }
    #[cfg(not(feature = "esp32"))]
    {
        Err(SystemEventError::UlpWakeupNotSupported)
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
    let event = SHUTDOWN_EVENT.lock().await;
    match *event {
        Some(SystemEvent::Restart) => terminate(),
        Some(SystemEvent::DeepSleep(duration)) => {
            #[cfg(feature = "esp32")]
            {
                if let Some(dur) = duration {
                    let dur_micros = dur.as_micros() as u64;
                    let result = unsafe { sys::esp_sleep_enable_timer_wakeup(dur_micros) };
                    if result != sys::ESP_OK {
                        log::error!("failed to enable timer wakeup: {:?}", result);
                    }
                }

                log::info!("{}", *event.clone().unwrap());
                unsafe {
                    sys::esp_deep_sleep_start();
                }
            }

            #[cfg(not(feature = "esp32"))]
            {
                if let Some(dur) = duration {
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
        }
        None => {
            log::error!("call to shutdown/restart without request to system, terminating");
            terminate()
        }
    }
}
