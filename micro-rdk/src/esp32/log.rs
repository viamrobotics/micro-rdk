//! This is for infrastructure supporting uploading logs from an ESP32 to Viam's cloud infrastructure
//! (see common/log.rs for more details). On an ESP32, there are two sources of logs: logs from ESP-IDF,
//! and logs from various Rust crates. The EspLogger from esp-idf-svc efficiently redirects the latter
//! logs to UART in a format matching the first, so it is sufficient to simply wrap it. However
//! to capture ESP-IDF logs, it is necessary to use the replace the default vprintf function to write to LOG_BUFFER
//! (again, see common/log.rs) before invoking the previously existing vprintf function in order to write to
//! UART. We store the previous vprintf function in PREVIOUS_LOGGER and use esp_log_set_vprintf for this purpose.
use crate::{
    common::log::ViamLogEntry,
    google::protobuf::{value::Kind, Struct, Value},
    proto::common::v1::LogEntry,
};

use esp_idf_svc::log::EspLogger;
use esp_idf_svc::sys::{esp_log_set_vprintf, va_list, vprintf_like_t};
use printf_compat::output::display;
use ringbuf::Rb;
use std::{collections::HashMap, sync::OnceLock};
use std::{ffi::c_char, sync::Mutex};

use crate::common::log::{get_log_buffer, ViamLogAdapter};

fn previous_logger() -> &'static Mutex<vprintf_like_t> {
    static PREVIOUS_LOGGER: OnceLock<Mutex<vprintf_like_t>> = OnceLock::new();
    PREVIOUS_LOGGER.get_or_init(|| Mutex::new(None))
}

fn current_log_statement() -> &'static Mutex<Vec<String>> {
    static CURRENT_LOG_STATEMENT: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    CURRENT_LOG_STATEMENT.get_or_init(|| Mutex::new(vec![]))
}

// A single log statement is often broken up into multiple calls to vprintf. So we store
// the fragments in CURRENT_LOG_STATEMENT. Detecting whether we have encountered the start
// of a new statement is futher complicated by the fact that, depending on the ESP-IDF component
// producing the log, the statement can be in one of the following formats
//
// "\x1b[0;<color_indicator>m<level_indicator> ... \x1b[0m"
//
// "<level_indicator> (<timestamp>) ..."
// This complication is reflected in the function below and its helper function
// process_current_statement_and_level
#[allow(improper_ctypes_definitions)]
unsafe extern "C" fn log_handler(arg1: *const c_char, arg2: va_list) -> i32 {
    let va_list: core::ffi::VaList = std::mem::transmute(&arg2);
    let fmt_message = display(arg1, va_list);
    let message = format!("{}", fmt_message).trim().to_string();
    let message_clone = message.clone();
    let start_of_new_statement = (message_clone.len() >= 3)
        && (matches!(&message_clone[..3], "I (" | "E (" | "W (" | "D (" | "V (")
            || message_clone.starts_with("\x1b[0;"));
    let mut current_fragments = current_log_statement().lock().unwrap();
    if start_of_new_statement && !current_fragments.is_empty() {
        let full_message = current_fragments.join(" ");
        let _ = get_log_buffer()
            .lock_blocking()
            .push_overwrite(process_current_statement_and_level(full_message));
        current_fragments.clear();
    }
    current_fragments.push(message_clone);
    if let Some(prev_logger) = *(previous_logger().lock().unwrap()) {
        prev_logger(arg1, arg2)
    } else {
        0
    }
}

fn process_current_statement_and_level(mut full_message: String) -> ViamLogEntry {
    let (mut message, level_initial) = if full_message.starts_with("\x1b[0;") {
        let stripped = full_message.split_off("\x1b[0;".len() + 3);
        let mut stripped_end = stripped
            .strip_suffix("\x1b[0m")
            .unwrap_or(stripped.as_str())
            .to_string();
        let stripped_without_level = stripped_end.split_off(2);
        (stripped_without_level, stripped_end)
    } else if full_message.len() > 1 {
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
        let mut guard = previous_logger().lock().unwrap();
        *guard = unsafe { esp_log_set_vprintf(Some(log_handler)) };
        self.initialize();
    }
    fn get_level_filter(&self) -> ::log::LevelFilter {
        self.get_max_level()
    }
    fn new() -> Self {
        Self {}
    }
}
