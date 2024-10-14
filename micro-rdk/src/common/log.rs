use crate::{
    google::protobuf::{value::Kind, Struct, Timestamp, Value},
    proto::common::v1::LogEntry,
};
use async_lock::Mutex as AsyncMutex;
use chrono::Local;
use ringbuf::{LocalRb, Rb};
use std::{
    collections::HashMap,
    mem::MaybeUninit,
    sync::OnceLock,
    time::{Duration, Instant},
};

use super::app_client::{AppClient, AppClientError, PeriodicAppClientTask};

// We need a static buffer of logs on the heap, but because we cannot guarantee that the current time has been set
// at every instance of logging, so we store each log alongside an instance of Instant. We assume that current time
// has been set on the system by the time an AppClient is available for uploading the logs and so use the Instant
// to correct the timestamp on the LogEntry.
pub(crate) struct ViamLogEntry {
    entry: LogEntry,
    time: Instant,
}

impl ViamLogEntry {
    pub(crate) fn from_record(record: &::log::Record<'_>) -> Self {
        Self {
            entry: record.into(),
            time: Instant::now(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn new(entry: LogEntry) -> Self {
        Self {
            entry,
            time: Instant::now(),
        }
    }

    fn get_time_corrected_entry(mut self) -> LogEntry {
        let time = Local::now().fixed_offset();
        let corrected_time = time - (Instant::now().duration_since(self.time));
        let secs = corrected_time.timestamp();
        let nanos = corrected_time.timestamp_subsec_nanos();
        let timestamp = Timestamp {
            seconds: secs,
            nanos: nanos as i32,
        };
        self.entry.time = Some(timestamp);
        self.entry
    }
}

impl From<&::log::Record<'_>> for LogEntry {
    fn from(value: &::log::Record) -> Self {
        LogEntry {
            host: "esp32".to_string(),
            level: value.level().as_str().to_string().to_lowercase(),
            time: None,
            logger_name: "viam-micro-server".to_string(),
            message: format!("{}", value.args()),
            caller: Some(Struct {
                fields: HashMap::from([
                    (
                        "Defined".to_string(),
                        Value {
                            kind: Some(Kind::BoolValue(true)),
                        },
                    ),
                    (
                        "File".to_string(),
                        Value {
                            kind: value.file().map(|f| Kind::StringValue(f.to_string())),
                        },
                    ),
                    (
                        "Line".to_string(),
                        Value {
                            kind: value.line().map(|l| Kind::NumberValue(l as f64)),
                        },
                    ),
                ]),
            }),
            stack: "".to_string(),
            fields: vec![],
        }
    }
}

type LogBufferType = LocalRb<ViamLogEntry, Vec<MaybeUninit<ViamLogEntry>>>;

// We've chosen a size of 150 for the buffer due to a roughly observed maximum of 200 bytes per log message and
// a desire to restrict the total amount of space for the cache to 30KB without losing logs to overwriting
// overwriting the ring buffer between uploads. The consequence is that, when the device is offline, we will
// cache the last 150 logs.
pub(crate) fn get_log_buffer() -> &'static AsyncMutex<LogBufferType> {
    static LOG_BUFFER: OnceLock<AsyncMutex<LogBufferType>> = OnceLock::new();
    LOG_BUFFER.get_or_init(|| AsyncMutex::new(LocalRb::new(150)))
}

pub(crate) struct LogUploadTask;

impl PeriodicAppClientTask for LogUploadTask {
    fn get_default_period(&self) -> std::time::Duration {
        Duration::from_secs(1)
    }
    fn name(&self) -> &str {
        "LogUpload"
    }
    fn invoke<'b, 'a: 'b>(
        &'a self,
        app_client: &'b AppClient,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<Duration>, AppClientError>> + 'b>,
    > {
        Box::pin(async move {
            let entries: Vec<LogEntry> = {
                let mut logs = get_log_buffer().lock().await;
                logs.pop_iter()
                    .map(|log_entry| log_entry.get_time_corrected_entry())
                    .collect()
            };
            if entries.is_empty() {
                Ok(None)
            } else {
                app_client.push_logs(entries).await.map(|_| None)
            }
        })
    }
}

pub trait ViamLogAdapter {
    fn before_log_setup(&self);
    fn get_level_filter(&self) -> ::log::LevelFilter;
    fn new() -> Self;
}

// ViamLogger is a wrapper around an existing logger that stores a copy into LOG_BUFFER for later
// upload to the cloud. The existing logger should satisfy log::Log and the ViamLogAdapter
// trait and then by initialized using this function at the start of main
pub fn initialize_logger<T: ::log::Log + ViamLogAdapter + 'static>() {
    let inner = T::new();
    let logger = ViamLogger::new(inner);
    let filter = logger.level_filter();
    logger.before_log_setup();
    let _ = ::log::set_boxed_logger(Box::new(logger));
    ::log::set_max_level(filter)
}

struct ViamLogger<L>(L);

impl<L> ViamLogger<L>
where
    L: ::log::Log + ViamLogAdapter,
{
    fn new(inner: L) -> Self {
        Self(inner)
    }

    fn before_log_setup(&self) {
        self.0.before_log_setup()
    }

    fn level_filter(&self) -> ::log::LevelFilter {
        self.0.get_level_filter()
    }
}

impl<L> ::log::Log for ViamLogger<L>
where
    L: ::log::Log + ViamLogAdapter,
{
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.0.enabled(metadata)
    }

    fn flush(&self) {
        self.0.flush()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            self.0.log(record);
            let mut buffer = get_log_buffer().lock_blocking();
            let _ = buffer.push_overwrite(ViamLogEntry::from_record(record));
        }
    }
}
