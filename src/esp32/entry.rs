#![allow(dead_code)]
use crate::common::app_client::{AppClientBuilder, AppClientConfig};
use crate::{
    common::grpc_client::GrpcClient, esp32::exec::Esp32Executor, esp32::tcp::Esp32Stream,
    esp32::tls::Esp32Tls,
};
use anyhow::Result;
use esp_idf_hal::task::{notify, wait_notification};
use esp_idf_sys::{vTaskDelete, xTaskCreatePinnedToCore, xTaskGetCurrentTaskHandle, TaskHandle_t};

use std::{ffi::c_void, time::Duration};

static CLIENT_TASK: &[u8] = b"client\0";

/// start the robot client
pub fn start(ip: AppClientConfig, handle: Option<TaskHandle_t>) -> Result<TaskHandle_t> {
    log::info!("starting up robot client");
    let ip = Box::into_raw(Box::new((ip, handle)));
    let mut hnd: TaskHandle_t = std::ptr::null_mut();
    let ret = unsafe {
        xTaskCreatePinnedToCore(
            Some(client_entry),                // C ABI compatible entry function
            CLIENT_TASK.as_ptr() as *const i8, // task name
            8192 * 3,                          // stack size
            ip as *mut c_void,                 // pass ip as argument
            20,                                // priority (low)
            &mut hnd,                          // we don't store the handle
            0,                                 // run it on core 0
        )
    };
    if ret != 1 {
        return Err(anyhow::anyhow!("wasn't able to create the client task"));
    }
    log::error!("got handle {:?}", hnd);
    Ok(hnd)
}

/// client main loop
fn clientloop(config: AppClientConfig) -> Result<()> {
    let mut tls = Box::new(Esp32Tls::new_client());
    let conn = tls.open_ssl_context(None)?;
    let conn = Esp32Stream::TLSStream(Box::new(conn));
    let executor = Esp32Executor::new();

    let grpc_client = GrpcClient::new(conn, executor, "https://app.viam.com:443")?;

    let mut _app_client = AppClientBuilder::new(grpc_client, config).build();

    log::error!("current task handle {:?}", unsafe {
        xTaskGetCurrentTaskHandle()
    });
    Ok(())
}

/// C compatible entry function
extern "C" fn client_entry(config: *mut c_void) {
    let config: Box<(AppClientConfig, Option<TaskHandle_t>)> =
        unsafe { Box::from_raw(config as *mut (AppClientConfig, Option<TaskHandle_t>)) };
    if let Some(err) = clientloop(config.0).err() {
        log::error!("client returned with error {}", err);
    }
    if let Some(hnd) = config.1 {
        unsafe {
            let _ = notify(hnd, 12345);
        }
    } else {
        loop {
            if let Some(_r) = wait_notification(Some(Duration::from_secs(30))) {
                log::info!("connection incomming the client task will stop");
                break;
            }
        }
    }
    unsafe {
        vTaskDelete(std::ptr::null_mut());
    }
}
