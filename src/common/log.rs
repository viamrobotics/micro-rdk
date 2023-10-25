use crate::google::protobuf::{value::Kind, Struct, Timestamp, Value};
use crate::proto::app::v1::LogEntry;
use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;

pub fn config_log_entry(time: DateTime<FixedOffset>, err: Option<&anyhow::Error>) -> LogEntry {
    let secs = time.timestamp();
    let nanos = time.timestamp_subsec_nanos();
    let level = match err {
        Some(_) => "error".to_string(),
        None => "info".to_string(),
    };
    let message = match err {
        Some(err) => format!("could not create robot from config: {err}"),
        None => "successfully created robot from config".to_string(),
    };
    LogEntry {
        host: "esp32".to_string(),
        level,
        time: Some(Timestamp {
            seconds: secs,
            nanos: nanos as i32,
        }),
        logger_name: "robot_server".to_string(),
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
    }
}
