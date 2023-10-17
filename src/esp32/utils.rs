#![allow(unused_imports)]
#![allow(unused_macros)]

#[macro_export]
macro_rules! esp32_print_heap_summary {
    () => {
        #[cfg(debug_assertions)]
        {
            use esp_idf_sys::{heap_caps_get_free_size, heap_caps_get_total_size, MALLOC_CAP_8BIT};
            let total = unsafe { heap_caps_get_total_size(MALLOC_CAP_8BIT) };
            let free = unsafe { heap_caps_get_free_size(MALLOC_CAP_8BIT) };
            log::info!("total heap {}, free {}", total, free);
        }
    };
}
pub use esp32_print_heap_summary;

macro_rules! esp32_print_heap_internal_summary {
    () => {
        #[cfg(debug_assertions)]
        {
            use esp_idf_sys::{
                heap_caps_get_free_size, heap_caps_get_total_size, MALLOC_CAP_8BIT,
                MALLOC_CAP_INTERNAL,
            };
            let total = unsafe { heap_caps_get_total_size(MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT) };
            let free = unsafe { heap_caps_get_free_size(MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT) };
            log::info!("internal heap {}, free {}", total, free);
        }
    };
}
pub(crate) use esp32_print_heap_internal_summary;

macro_rules! esp32_print_heap_spiram_summary {
    () => {
        #[cfg(debug_assertions)]
        {
            use esp_idf_sys::{
                heap_caps_get_free_size, heap_caps_get_total_size, MALLOC_CAP_8BIT,
                MALLOC_CAP_SPIRAM,
            };
            let total = unsafe { heap_caps_get_total_size(MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT) };
            let free = unsafe { heap_caps_get_free_size(MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT) };
            log::info!("internal heap {}, free {}", total, free);
        }
    };
}
pub(crate) use esp32_print_heap_spiram_summary;

macro_rules! esp32_print_stack_high_watermark {
    () => {
        #[cfg(debug_assertions)]
        {
            use esp_idf_sys::uxTaskGetStackHighWaterMark;
            log::info!("stack high watermark is {:#X}", unsafe {
                uxTaskGetStackHighWaterMark(std::ptr::null_mut())
            });
        }
    };
}

pub(crate) use esp32_print_stack_high_watermark;
