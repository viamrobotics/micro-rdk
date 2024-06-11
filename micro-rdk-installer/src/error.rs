use espflash::error::Error as EspFlashError;
use reqwest::Error as RequestError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("NVS Data Processing Error: {0}")]
    NVSDataProcessingError(String),
    #[error("NVS entry missing in partition table")]
    NVSMissingError,
    #[error("NVS offset missing in partition table")]
    NVSOffsetMissingError,
    #[error("File Error: {0}")]
    FileError(std::io::Error),
    #[error("Binary Retrieval Error: {0}")]
    BinaryRetrievalError(RequestError),
    #[error("Binary Too Small, File Size: {0}")]
    BinaryEditError(u64),
    #[error("Binary Too Large, File Size: {0}")]
    BinaryBufferError(u64),
    #[error("{0}")]
    WifiPasswordTooLongError(String),
    #[error("Wifi Credentials Error: {0}")]
    WifiCredentialsError(std::io::Error),
    #[error("Config Parsing Error: {0}")]
    ConfigParseError(serde_json::Error),
    #[error("Async Runtime Error: {0}")]
    AsyncError(std::io::Error),
    #[error("Missing Config Info: {0}")]
    MissingConfigInfo(String),
    #[error("Config Request Error: {0}")]
    ConfigRequestError(String),
    #[error("Certificate Request Error: {0}")]
    CertificateRequestError(String),
    #[error("App Connection Error: {0}")]
    AppConnectionError(anyhow::Error),
    #[error("Flash connection error")]
    FlashConnect,
    #[error("EspFlash Flash error: {0}")]
    EspFlashError(EspFlashError),
    #[error("Monitor serial error: {0}")]
    MonitorError(String),
    #[error("Unimplemented command: {0}")]
    UnimplementedError(String),
    #[error("Serial config error: {0}")]
    SerialConfigError(String),
    #[error("Partition Table Error: {0}")]
    PartitionTableError(String),
    #[error("No command received")]
    NoCommandError,
}

impl From<EspFlashError> for Error {
    fn from(value: EspFlashError) -> Self {
        Self::EspFlashError(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::ConfigParseError(value)
    }
}
