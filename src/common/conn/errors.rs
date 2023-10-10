use std::error::Error;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("couldn't open ssl connection")]
    ServerErrorOpenSslConnection,
    #[error("timeout while connecting")]
    ServerConnectionTimeout,
    #[error(transparent)]
    Other(#[from] Box<dyn Error + Send + Sync>),
    #[error("not configured")]
    ServerConnectionNotConfigured,
}
