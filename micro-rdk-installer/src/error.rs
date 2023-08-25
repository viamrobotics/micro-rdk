use rcgen::RcgenError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("NVS Data Processing Error: {0}")]
    NVSDataProcessingError(String),
    #[error("File Error: {0}")]
    FileError(std::io::Error),
    #[error("Binary Too Small, File Size: {0}")]
    BinaryEditError(u64),
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
    #[error("Error producing robot dtls certificates: {0}")]
    DtlsCertificateError(RcgenError),
    #[error("Config Request Error: {0}")]
    ConfigRequestError(String),
    #[error("Certificate Request Error: {0}")]
    CertificateRequestError(String),
    #[error("App Connection Error: {0}")]
    AppConnectionError(anyhow::Error),
    #[error("Unimplemented command: {0}")]
    UnimplementedError(String),
    #[error("No command received")]
    NoCommandError,
}

impl From<RcgenError> for Error {
    fn from(value: RcgenError) -> Self {
        Self::DtlsCertificateError(value)
    }
}

// impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::ConfigParseError(value)
    }
}
