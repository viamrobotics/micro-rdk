use std::error::Error;
use thiserror::Error;

use crate::common::{app_client::AppClientError, webrtc::api::WebRtcError};
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
    ServerAppClientError(AppClientError),
    #[error(transparent)]
    ServerWebRTCError(WebRtcError),
}
