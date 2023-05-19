use esp_idf_sys::{pcnt_isr_service_install, pcnt_isr_service_uninstall, EspError, ESP_OK};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/*
This module exists because we want to ensure uniqueness of unit number
across instances of an Esp32 Pulse Counter unit and make sure the isr service
is only installed once.

THIS MODULE IS A TEMPORARY MEASURE. When abstracting the atomicity of Esp32
peripherals to board, this logic should be moved there.

TODO: v5 of ESP-IDF has refactored pulse counter to manage what this module
accomplishes for us. Potentially only use this module when on chips on v4.

*/

lazy_static::lazy_static! {
    static ref NEXT_UNIT: Arc<AtomicU32> = Arc::new(AtomicU32::new(0));

    static ref ISR_INSTALLED: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    static ref NUMBER_OF_UNITS: Arc<AtomicU32> = Arc::new(AtomicU32::new(0));
}

pub(crate) fn get_unit() -> anyhow::Result<u32> {
    NUMBER_OF_UNITS.fetch_add(0, Ordering::Relaxed);
    Ok(NEXT_UNIT.fetch_add(1, Ordering::SeqCst))
}

pub(crate) fn isr_install() -> anyhow::Result<()> {
    if !ISR_INSTALLED.fetch_or(true, Ordering::SeqCst) {
        unsafe {
            match pcnt_isr_service_install(0) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
    }
    Ok(())
}

pub(crate) fn isr_installed() -> bool {
    ISR_INSTALLED.load(Ordering::SeqCst)
}

pub(crate) fn isr_remove_unit() {
    if NUMBER_OF_UNITS.fetch_sub(1, Ordering::Relaxed) <= 1
        && ISR_INSTALLED.fetch_xor(false, Ordering::SeqCst)
    {
        unsafe {
            pcnt_isr_service_uninstall();
        }
    };
}
