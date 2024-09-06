use crate::common::log::ViamLogAdapter;

impl ViamLogAdapter for env_logger::Logger {
    fn before_log_setup(&self) {}
    fn get_level_filter(&self) -> ::log::LevelFilter {
        self.filter()
    }
    fn new() -> Self {
        env_logger::builder()
            .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
            .build()
    }
}
