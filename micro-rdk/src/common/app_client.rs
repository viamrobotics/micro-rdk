#![allow(unused)]
use crate::proto::app::v1::CertificateRequest;
use crate::proto::app::v1::CertificateResponse;
use crate::proto::{
    app::v1::{
        AgentInfo, ConfigRequest, ConfigResponse, LogRequest, NeedsRestartRequest,
        NeedsRestartResponse,
    },
    common::v1::LogEntry,
    rpc::{
        v1::{AuthenticateRequest, AuthenticateResponse, Credentials},
        webrtc::v1::{AnswerRequest, AnswerResponse, AnswerResponseErrorStage},
    },
};
use bytes::{BufMut, Bytes, BytesMut};
use chrono::{format::ParseError, DateTime, Datelike, FixedOffset, Local};
use futures_lite::{Future, StreamExt};
use http_body_util::BodyExt;
use http_body_util::Full;
use http_body_util::StreamBody;
use hyper::{body::Frame, http::HeaderValue};
use prost::{DecodeError, EncodeError, Message};
use std::{
    net::Ipv4Addr,
    pin::Pin,
    rc::Rc,
    time::{Duration, SystemTime},
};
use thiserror::Error;

use super::conn::network::Network;
use super::credentials_storage::RobotCredentials;
use super::{
    grpc_client::{GrpcClient, GrpcClientError, GrpcMessageSender, GrpcMessageStream},
    webrtc::{
        api::{WebRtcApi, WebRtcError},
        certificate::Certificate,
        dtls::DtlsConnector,
        exec::WebRtcExecutor,
    },
};

#[cfg(feature = "data")]
use crate::proto::app::data_sync::v1::DataCaptureUploadRequest;

pub const VIAM_FOUNDING_YEAR: i32 = 2020;

#[derive(Error, Debug)]
pub enum AppClientError {
    #[error("wrong credentials")]
    AppWrongCredentials,
    #[error(transparent)]
    AppEncodeError(#[from] EncodeError),
    #[error(transparent)]
    AppDecodeError(#[from] DecodeError),
    #[error(transparent)]
    AppWebRtcError(#[from] WebRtcError),
    #[error("error converting from HeaderValue to string for 'date'")]
    AppConfigHeaderValueParseError,
    #[error(transparent)]
    AppConfigHeaderDateParseError(#[from] ParseError),
    #[error("Date missing from header of config response")]
    AppConfigHeaderDateMissingError,
    #[error(transparent)]
    AppGrpcClientError(#[from] GrpcClientError),
    #[error("request timeout")]
    AppClientRequestTimeout,
    #[error("empty body")]
    AppClientEmptyBody,
    #[error(transparent)]
    AppClientIoError(#[from] std::io::Error),
}

impl AppClientError {
    pub fn is_io_error(&self) -> bool {
        if let AppClientError::AppClientIoError(_) = self {
            return true;
        }
        false
    }
    pub fn is_permission_denied(&self) -> bool {
        if let AppClientError::AppGrpcClientError(GrpcClientError::GrpcError { code, .. }) = self {
            if *code == 7 {
                return true;
            }
        }
        false
    }
    pub fn is_unauthenticated(&self) -> bool {
        if let AppClientError::AppGrpcClientError(GrpcClientError::GrpcError { code, .. }) = self {
            if *code == 16 {
                return true;
            }
        }
        false
    }
}

pub struct AppClientBuilder {
    grpc_client: Box<GrpcClient>,
    robot_credentials: RobotCredentials,
}

pub(crate) fn encode_request<T>(req: T) -> Result<Bytes, AppClientError>
where
    T: Message,
{
    let mut buf = BytesMut::with_capacity(req.encoded_len() + 5);
    buf.put_u8(0);
    buf.put_u32(req.encoded_len() as u32);

    let mut msg = buf.split_off(5);
    req.encode(&mut msg)
        .map_err(AppClientError::AppEncodeError)?;
    buf.unsplit(msg);

    Ok(buf.into())
}

impl AppClientBuilder {
    /// Create a new AppClientBuilder
    pub fn new(grpc_client: Box<GrpcClient>, robot_credentials: RobotCredentials) -> Self {
        Self {
            grpc_client,
            robot_credentials,
        }
    }

    /// Consume the AppClientBuilder and returns an AppClient. This function will panic if
    /// the received config doesn't contain an fqdn field.
    pub async fn build(mut self) -> Result<AppClient, AppClientError> {
        let jwt = self.get_jwt_token().await?;

        Ok(AppClient {
            grpc_client: self.grpc_client.into(),
            jwt,
            robot_credentials: self.robot_credentials,
        })
    }
    pub async fn get_jwt_token(&mut self) -> Result<String, AppClientError> {
        let cred = Credentials {
            r#type: "robot-secret".to_owned(),
            payload: self.robot_credentials.robot_secret.clone(),
        };

        let req = AuthenticateRequest {
            entity: self.robot_credentials.robot_id.clone(),
            credentials: Some(cred),
        };

        let body = encode_request(req)?;
        let mut r = self
            .grpc_client
            .build_request(
                "/proto.rpc.v1.AuthService/Authenticate",
                None,
                "",
                Full::new(body).map_err(|never| match never {}).boxed(),
            )
            .map_err(AppClientError::AppGrpcClientError)?;

        let mut r = self
            .grpc_client
            .send_request(r)
            .await
            .map_err(AppClientError::AppGrpcClientError)?
            .0;

        if r.is_empty() {
            return Err(AppClientError::AppClientEmptyBody);
        }
        let r = r.split_off(5);
        let r = AuthenticateResponse::decode(r).map_err(AppClientError::AppDecodeError)?;
        Ok(format!("Bearer {}", r.access_token))
    }
}

#[derive(Clone)]
pub struct AppClient {
    robot_credentials: RobotCredentials,
    jwt: String,
    grpc_client: Rc<GrpcClient>,
}

pub(crate) struct AppSignaling {
    pub(crate) tx: GrpcMessageSender<AnswerResponse>,
    pub(crate) rx: GrpcMessageStream<AnswerRequest>,
}

impl AppClient {
    pub async fn get_certificates(&self) -> Result<CertificateResponse, AppClientError> {
        let req = CertificateRequest {
            id: self.robot_credentials.robot_id.clone(),
        };
        let body = encode_request(req)?;
        let r = self
            .grpc_client
            .build_request(
                "/viam.app.v1.RobotService/Certificate",
                Some(&self.jwt),
                "",
                BodyExt::boxed(Full::new(body).map_err(|never| match never {})),
            )
            .map_err(AppClientError::AppGrpcClientError)?;

        let (mut r, headers) = self.grpc_client.send_request(r).await?;
        if r.is_empty() {
            return Err(AppClientError::AppClientEmptyBody);
        }
        let r = r.split_off(5);
        Ok(CertificateResponse::decode(r)?)
    }

    pub(crate) fn initiate_signaling(
        &self,
        rpc_host: String,
    ) -> impl Future<Output = Result<AppSignaling, AppClientError>> {
        let (sender, receiver) = async_channel::bounded::<Bytes>(1);
        let r = self.grpc_client.build_request(
            "/proto.rpc.webrtc.v1.SignalingService/Answer",
            Some(&self.jwt),
            &rpc_host,
            BodyExt::boxed(StreamBody::new(receiver.map(|b| Ok(Frame::data(b))))),
        );

        let grpc_client = self.grpc_client.clone();
        async move {
            // insert a {"heartbeats-allowed": "true"} metadata key-value pair to
            // indicate to signaling server that we can receive heartbeats.
            let mut r = r?;
            r.headers_mut()
                .insert("heartbeats-allowed", HeaderValue::from_static("true"));

            let (tx, rx) = grpc_client
                .send_request_bidi::<AnswerResponse, AnswerRequest>(r, sender)
                .await
                .map_err(AppClientError::AppGrpcClientError)?;
            Ok(AppSignaling { tx, rx })
        }
    }

    pub fn robot_credentials(&self) -> RobotCredentials {
        self.robot_credentials.clone()
    }

    // returns both a response from the robot config request and the timestamp of the response
    // taken from its header for the purposes of timestamping configuration logs and returning
    // `last_reconfigured` values for resource statuses.
    pub async fn get_app_config(
        &self,
        ip: Option<Ipv4Addr>,
    ) -> Result<(Box<ConfigResponse>, Option<DateTime<FixedOffset>>), AppClientError> {
        let agent = ip.map(|ip| AgentInfo {
            os: "esp32".to_string(),
            host: "esp32".to_string(),
            ips: vec![ip.to_string()],
            version: env!("CARGO_PKG_VERSION").to_string(),
            git_revision: "".to_string(),
            platform: Some("esp32".to_string()),
        });

        let req = ConfigRequest {
            agent_info: agent,
            id: self.robot_credentials.robot_id.clone(),
        };
        let body = encode_request(req)?;

        let r = self
            .grpc_client
            .build_request(
                "/viam.app.v1.RobotService/Config",
                Some(&self.jwt),
                "",
                BodyExt::boxed(Full::new(body).map_err(|never| match never {})),
            )
            .map_err(AppClientError::AppGrpcClientError)?;

        let (mut r, headers) = self.grpc_client.send_request(r).await?;

        let datetime = if let Some(date_val) = headers.get("date") {
            let date_str = date_val
                .to_str()
                .map_err(|_| AppClientError::AppConfigHeaderValueParseError)?;
            DateTime::parse_from_rfc2822(date_str).ok()
        } else {
            None
        };

        #[cfg(feature = "esp32")]
        {
            // If the current datetime has not already been set, we use the datetime from
            // the config response to set the time of day on the device. This may be replaced
            // by calls to an NTP server in the future
            let local_dt = Local::now().fixed_offset();
            // Viam does not pre-exist the year 2020, so if the year is before that
            // at the very least the current time is wrong and needs to be corrected
            if local_dt.year() < VIAM_FOUNDING_YEAR {
                if let Some(current_dt) = datetime {
                    use esp_idf_svc::sys::{settimeofday, timeval};
                    let tv_sec = current_dt.timestamp() as i32;
                    let tv_usec = current_dt.timestamp_subsec_micros() as i32;
                    let current_timeval = timeval { tv_sec, tv_usec };
                    crate::esp32::esp_idf_svc::sys::esp!(unsafe {
                        settimeofday(&current_timeval as *const timeval, std::ptr::null())
                    })
                    .inspect_err(|err| {
                        log::error!(
                            "could not set time of day for timestamp {:?}: {:?}",
                            current_dt,
                            err
                        );
                    });
                }
            }
        }

        if r.is_empty() {
            return Err(AppClientError::AppClientEmptyBody);
        }
        let cfg_response = ConfigResponse::decode(r.split_off(5))?;

        Ok((Box::new(cfg_response), datetime))
    }

    pub async fn push_logs(&self, logs: Vec<LogEntry>) -> Result<(), AppClientError> {
        let req = LogRequest {
            id: self.robot_credentials.robot_id.clone(),
            logs,
        };

        let body = encode_request(req)?;
        let r = self
            .grpc_client
            .build_request(
                "/viam.app.v1.RobotService/Log",
                Some(&self.jwt),
                "",
                BodyExt::boxed(Full::new(body).map_err(|never| match never {})),
            )
            .map_err(AppClientError::AppGrpcClientError)?;
        self.grpc_client.send_request(r).await?;

        Ok(())
    }

    #[cfg(feature = "data")]
    pub async fn upload_data(
        &self,
        data_req: DataCaptureUploadRequest,
    ) -> Result<(), AppClientError> {
        let body = encode_request(data_req)?;
        let r = self
            .grpc_client
            .build_request(
                "/viam.app.datasync.v1.DataSyncService/DataCaptureUpload",
                Some(&self.jwt),
                "",
                BodyExt::boxed(Full::new(body).map_err(|never| match never {})),
            )
            .map_err(AppClientError::AppGrpcClientError)?;
        self.grpc_client.send_request(r).await?;

        Ok(())
    }

    /// Obtains the Duration for which we should wait before next
    /// checking for a restart. If no Duration is returned, then the
    /// app has signaled that we should restart now.
    pub async fn check_for_restart(&self) -> Result<Option<Duration>, AppClientError> {
        let req = NeedsRestartRequest {
            id: self.robot_credentials.robot_id.clone(),
        };
        let body = encode_request(req)?;
        let r = self
            .grpc_client
            .build_request(
                "/viam.app.v1.RobotService/NeedsRestart",
                Some(&self.jwt),
                "",
                BodyExt::boxed(Full::new(body).map_err(|never| match never {})),
            )
            .map_err(AppClientError::AppGrpcClientError)?;
        let (mut response, headers_) = self.grpc_client.send_request(r).await?;
        if response.is_empty() {
            return Err(AppClientError::AppClientEmptyBody);
        }
        let response = NeedsRestartResponse::decode(response.split_off(5))?;

        const MIN_RESTART_DURATION: Duration = Duration::from_secs(1);
        const DEFAULT_RESTART_DURATION: Duration = Duration::from_secs(5);

        // If app replied with `must_restart` true, then return `None` to indicate that restart was
        // requested. Otherwise, if app replied with populated and sensible restart interval, return
        // that. Failing that, return the default timeout.
        Ok(match response.must_restart {
            true => None,
            false => match response.restart_check_interval {
                None => Some(DEFAULT_RESTART_DURATION),
                Some(d) => Some(match Duration::try_from(d) {
                    Ok(d) => d.max(MIN_RESTART_DURATION),
                    Err(e) => DEFAULT_RESTART_DURATION,
                }),
            },
        })
    }

    /// Returns the strong count of the AppClient's internal
    /// gRPC client, serving as a rough proxy for the number of
    /// concurrent tasks using this app client.
    pub(crate) fn get_grpc_client_count(&self) -> usize {
        Rc::strong_count(&self.grpc_client)
    }
}

impl Drop for AppClient {
    fn drop(&mut self) {
        log::debug!("dropping AppClient")
    }
}

/// An object-safe trait for use with `ViamServerBuilder::with_periodic_app_client_task`. An object
/// implementing this trait represents a periodic activity to be performed against an `AppClient`,
/// such as checking for restarts or uploading cached data to the data service.
pub trait PeriodicAppClientTask {
    /// Returns the name of this task, primarily for inclusion in error messages or logging.
    fn name(&self) -> &str;

    /// Returns a Duration to indicate how frequently the task would like to be invoked. The
    /// `ViamServer` may adjust the actual `Duration` between invocations based on the return value
    /// of `invoke`, so that services which negotiate a frequency with app can honor the request.
    fn get_default_period(&self) -> Duration;

    /// A desugared `async fn` (so we can declare it in a trait) which will be periodically invoked
    /// by the `ViamServer` per the currently negotiated `Duration`.
    fn invoke<'b, 'a: 'b>(
        &'a self,
        app_client: &'b AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'b>>;
}

impl<T: PeriodicAppClientTask + ?Sized> PeriodicAppClientTask for Box<T> {
    fn get_default_period(&self) -> Duration {
        (**self).get_default_period()
    }
    fn invoke<'b, 'a: 'b>(
        &'a self,
        app_client: &'b AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'b>> {
        (**self).invoke(app_client)
    }
    fn name(&self) -> &str {
        (**self).name()
    }
}
