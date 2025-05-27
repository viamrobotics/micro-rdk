use std::error::Error;
use thiserror::Error;

use crate::common::{
    app_client::AppClientError,
    webrtc::{api::WebRtcError, dtls::DtlsError},
};
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
    #[error(transparent)]
    ServerAppClientError(#[from] AppClientError),
    #[error(transparent)]
    ServerWebRTCError(#[from] WebRtcError),
    #[error(transparent)]
    ServerIoError(#[from] std::io::Error),
    #[error(transparent)]
    ServerDtlsError(#[from] DtlsError),
    #[error("tasks completed but no restart or shutdown was requested")]
    ServerInvalidCompletedState,
    #[error("task did not finish in time, will be force quit")]
    ServerTaskShutdownTimeout,
}
