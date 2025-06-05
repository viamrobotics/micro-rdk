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
    DeepSleep {
        duration: Option<Duration>,
        ulp_enabled: bool,
    },
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
                Self::DeepSleep {
                    duration,
                    ulp_enabled,
                } => {
                    let mut s = String::new();
                    s.push_str(
                        &duration
                            .as_ref()
                            .map_or("enter deep sleep".to_string(), |d| {
                                format!("enter deep sleep for {} microseconds", d.as_micros())
                            }),
                    );
                    if *ulp_enabled {
                        s.push_str("- ulp mode enabled");
                    }
                    s
                }
            }
        )
    }
}

pub(crate) async fn send_system_event(
    event: SystemEvent,
    force: bool,
) -> Result<(), SystemEventError> {
    log::info!("received call to {}", event);
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
        Some(SystemEvent::DeepSleep {
            duration,
            ulp_enabled,
        }) => {
            #[cfg(feature = "esp32")]
            {
                let mut result: sys::esp_err_t;

                // disable other wakeup sources before setting
                unsafe {
                    result = sys::esp_sleep_disable_wakeup_source(
                        sys::esp_sleep_source_t_ESP_SLEEP_WAKEUP_ALL,
                    );
                }

                if result != sys::ESP_OK {
                    log::error!("failed to clear wakeup sources before setting: {}", result);
                }

                if ulp_enabled {
                    log::info!("enabling ULP wakeup");

                    unsafe {
                        result = sys::esp_sleep_enable_ulp_wakeup();
                    }

                    match result {
                        sys::ESP_OK => {
                            log::info!("ULP wakeup enabled");
                        }
                        sys::ESP_ERR_NOT_SUPPORTED => {
                            log::error!("additional current by touch enabled");
                        }
                        sys::ESP_ERR_INVALID_STATE => {
                            log::error!("co-processor not enabled or wakeup trigger conflicts with ulp wakeup");
                        }
                        _ => log::error!("failed to enable ULP: {:?}", result),
                    }
                }

                if let Some(dur) = duration {
                    let dur_micros = dur.as_micros() as u64;
                    unsafe {
                        result = sys::esp_sleep_enable_timer_wakeup(dur_micros);
                    }
                    if result != sys::ESP_OK {
                        unreachable!("duration requested too long")
                    }
                    log::warn!("Esp32 entering deep sleep for {} microseconds!", dur_micros);
                }

                unsafe {
                    sys::esp_deep_sleep_start();
                }
            }

            #[cfg(not(feature = "esp32"))]
            {
                if ulp_enabled {
                    log::info!("simulating setting ulp wakeup");
                }
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
