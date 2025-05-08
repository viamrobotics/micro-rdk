use std::sync::{Arc, LazyLock};
use std::time::Duration;

use async_lock::Mutex as AsyncMutex;

use super::app_client::{AppClient, PeriodicAppClientTask};
use super::log::LogUploadTask;
use super::runtime::terminate;

pub(crate) static SHUTDOWN_EVENT: LazyLock<Arc<AsyncMutex<Option<SystemEvent>>>> =
    LazyLock::new(|| Arc::new(AsyncMutex::new(None)));

pub(crate) enum SystemEvent {
    Restart,
    // LightSleep(Duration),
    #[allow(dead_code)]
    DeepSleep(Option<Duration>),
}

pub(crate) async fn send_system_change(event: SystemEvent) {
    log::info!("system event set");
    let _ = SHUTDOWN_EVENT.lock().await.insert(event);
    // SHUTDOWN_REQUESTED.store(true, Ordering::Relaxed);
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
        }
        None => {}
    }
}
