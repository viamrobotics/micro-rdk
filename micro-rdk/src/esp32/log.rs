//! This is for infrastructure supporting uploading logs from an ESP32 to Viam's cloud infrastructure
//! (see common/log.rs for more details). On an ESP32, there are two sources of logs: logs from ESP-IDF,
//! and logs from various Rust crates. The EspLogger from esp-idf-svc efficiently redirects the latter
//! logs to UART in a format matching the first, so it is sufficient to simply wrap it. However
//! to capture ESP-IDF logs, it is necessary to replace the default vprintf function to write to LOG_BUFFER
//! (again, see common/log.rs) before invoking the previously existing vprintf function in order to write to
//! UART. We store the previous vprintf function in PREVIOUS_LOGGER and use esp_log_set_vprintf for this purpose.
//! The capture of ESP-IDF logs is only available with the "esp-idf-logs" feature.
#[cfg(feature = "esp-idf-logs")]
use crate::{
    common::log::{get_log_buffer, ViamLogEntry},
    google::protobuf::{value::Kind, Struct, Value},
    proto::common::v1::LogEntry,
};

use esp_idf_svc::log::EspLogger;
#[cfg(feature = "esp-idf-logs")]
use esp_idf_svc::sys::{esp_log_set_vprintf, va_list, vprintf_like_t};
#[cfg(feature = "esp-idf-logs")]
use printf_compat::output::display;
#[cfg(feature = "esp-idf-logs")]
use ringbuf::Rb;
#[cfg(feature = "esp-idf-logs")]
use std::{collections::HashMap, ffi::CString, sync::OnceLock};
#[cfg(feature = "esp-idf-logs")]
use std::{ffi::c_char, sync::Mutex};

use crate::common::log::ViamLogAdapter;

#[cfg(feature = "esp-idf-logs")]
static PREVIOUS_LOGGER: OnceLock<vprintf_like_t> = OnceLock::new();

#[cfg(feature = "esp-idf-logs")]
const MESSAGE_START: &str = "\x1b[0;";
#[cfg(feature = "esp-idf-logs")]
const MESSAGE_END: &str = "\x1b[0m";

// Detecting whether we have encountered the start of a new statement is complicated
// by the fact that, depending on the ESP-IDF component producing the log, the statement
// can be in one of the following formats.
//
// "\x1b[0;<color_indicator>m<level_indicator> ... \x1b[0m"
//
// "<level_indicator> (<timestamp>) ..."
//
// This complication is reflected in the function below and its helper function
// process_current_statement_and_level. Additionally, any IDF component that does not
// properly call ESP_LOGx (for instance, esp-wifi) will have some if its logs uploaded in
// segments (to be potentially revisited)
#[cfg(feature = "esp-idf-logs")]
#[allow(improper_ctypes_definitions)]
unsafe extern "C" fn log_handler(arg1: *const c_char, arg2: va_list) -> i32 {
    let va_list: core::ffi::VaList = std::mem::transmute(&arg2);
    let fmt_message = display(arg1, va_list);
    let message = format!("{}", fmt_message).to_string();
    let _ = get_log_buffer()
        .lock_blocking()
        .push_overwrite(process_current_statement_and_level(message.clone()));
    if let Some(prev_logger) = PREVIOUS_LOGGER.get().unwrap_or(&None) {
        let fmt_c_str = CString::new(message).unwrap();
        prev_logger(fmt_c_str.as_ptr() as *const c_char, [0; 3])
    } else {
        0
    }
}

#[cfg(feature = "esp-idf-logs")]
fn process_current_statement_and_level(mut full_message: String) -> ViamLogEntry {
    let (mut message, level_initial) = if full_message.starts_with(MESSAGE_START) {
        let stripped = full_message.split_off(MESSAGE_START.len() + 3);
        let mut stripped_end = stripped
            .strip_suffix(MESSAGE_END)
            .unwrap_or(stripped.as_str())
            .to_string();
        let stripped_without_level = stripped_end.split_off(2);
        (stripped_without_level, stripped_end)
    } else if (full_message.len() >= 3)
        && matches!(&full_message[..3], "I (" | "E (" | "W (" | "D (" | "V (")
    {
        let stripped_message = full_message.split_off(2);
        (stripped_message, full_message)
    } else {
        (full_message, "U".to_string())
    };
    let level = match level_initial.as_str() {
        "I " => "info",
        "E " => "error",
        "W " => "warn",
        "D " => "debug",
        "V " => "trace",
        &_ => "unknown",
    }
    .to_string();
    if level.as_str() != "unknown" {
        // we strip the ESP-IDF timestamp in favor of our corrected one
        if let Some(end_of_timestamp) = message.find(')') {
            message = message[(end_of_timestamp + 1)..].to_string()
        }
    }
    ViamLogEntry::new(LogEntry {
        host: "esp32".to_string(),
        level,
        time: None,
        logger_name: "viam-micro-server".to_string(),
        message,
        caller: Some(Struct {
            fields: HashMap::from([(
                "Defined".to_string(),
                Value {
                    kind: Some(Kind::BoolValue(false)),
                },
            )]),
        }),
        stack: "".to_string(),
        fields: vec![],
    })
}

impl ViamLogAdapter for EspLogger {
    fn before_log_setup(&self) {
        #[cfg(feature = "esp-idf-logs")]
        let _ = PREVIOUS_LOGGER.get_or_init(|| unsafe { esp_log_set_vprintf(Some(log_handler)) });
        self.initialize();
    }
    fn get_level_filter(&self) -> ::log::LevelFilter {
        self.get_max_level()
    }
    fn new() -> Self {
        EspLogger::new()
    }
}
