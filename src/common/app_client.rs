#![allow(unused)]
use anyhow::Context;
use bytes::{BufMut, Bytes, BytesMut};
use chrono::{format::ParseError, DateTime, FixedOffset};
use futures_lite::Future;
use prost::{DecodeError, EncodeError, Message};
use std::{net::Ipv4Addr, pin::Pin, rc::Rc, time::SystemTime};
use thiserror::Error;

use crate::proto::{
    app::v1::{AgentInfo, ConfigRequest, ConfigResponse, LogEntry, LogRequest},
    rpc::{
        v1::{AuthenticateRequest, AuthenticateResponse, Credentials},
        webrtc::v1::{AnswerRequest, AnswerResponse, AnswerResponseErrorStage},
    },
};

use super::{
    grpc_client::{GrpcClient, GrpcMessageSender, GrpcMessageStream},
    webrtc::{
        api::{WebRtcApi, WebRtcError},
        certificate::Certificate,
        dtls::DtlsConnector,
        exec::WebRtcExecutor,
    },
};

#[derive(Error, Debug)]
pub enum AppClientError {
    #[error("wrong credentials")]
    AppWrongCredentials,
    #[error(transparent)]
    AppOtherError(#[from] anyhow::Error),
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
}

#[derive(Debug, Clone)]
pub struct AppClientConfig {
    robot_id: String,
    robot_secret: String,
    ip: Ipv4Addr,
    rpc_host: String,
}

impl Default for AppClientConfig {
    fn default() -> Self {
        Self {
            robot_id: "".to_owned(),
            robot_secret: "".to_owned(),
            ip: Ipv4Addr::new(0, 0, 0, 0),
            rpc_host: "".to_owned(),
        }
    }
}

impl AppClientConfig {
    pub fn new(robot_secret: String, robot_id: String, ip: Ipv4Addr, rpc_host: String) -> Self {
        AppClientConfig {
            robot_id,
            robot_secret,
            ip,
            rpc_host,
        }
    }
    pub fn get_robot_id(&self) -> String {
        self.robot_id.clone()
    }
    pub fn get_ip(&self) -> Ipv4Addr {
        self.ip
    }
    pub fn set_rpc_host(&mut self, rpc_host: String) {
        self.rpc_host = rpc_host
    }
}

pub struct AppClientBuilder<'a> {
    grpc_client: Box<GrpcClient<'a>>,
    config: AppClientConfig,
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

impl<'a> AppClientBuilder<'a> {
    /// Create a new AppClientBuilder
    pub fn new(grpc_client: Box<GrpcClient<'a>>, config: AppClientConfig) -> Self {
        Self {
            grpc_client,
            config,
        }
    }
    /// Consume the AppClientBuilder and returns an AppClient. This function will panic if
    /// the received config doesn't contain an fqdn field.
    pub fn build(mut self) -> Result<AppClient<'a>, AppClientError> {
        let jwt = self.get_jwt_token()?;

        Ok(AppClient {
            grpc_client: self.grpc_client,
            jwt,
            ip: self.config.ip,
            config: self.config,
        })
    }
    pub fn get_jwt_token(&mut self) -> Result<String, AppClientError> {
        let r = self
            .grpc_client
            .build_request("/proto.rpc.v1.AuthService/Authenticate", None, "")
            .map_err(AppClientError::AppOtherError)?;

        let cred = Credentials {
            r#type: "robot-secret".to_owned(),
            payload: self.config.robot_secret.clone(),
        };

        let req = AuthenticateRequest {
            entity: self.config.robot_id.clone(),
            credentials: Some(cred),
        };

        let body = encode_request(req)?;

        let mut r = self
            .grpc_client
            .send_request(r, body)
            .map_err(AppClientError::AppOtherError)?
            .0;
        let r = r.split_off(5);
        let r = AuthenticateResponse::decode(r).map_err(AppClientError::AppDecodeError)?;
        Ok(format!("Bearer {}", r.access_token))
    }
}

pub struct AppClient<'a> {
    config: AppClientConfig,
    jwt: String,
    grpc_client: Box<GrpcClient<'a>>,
    ip: Ipv4Addr,
}

pub(crate) struct AppSignaling(
    pub(crate) GrpcMessageSender<AnswerResponse>,
    pub(crate) GrpcMessageStream<AnswerRequest>,
);

impl<'a> AppClient<'a> {
    pub(crate) async fn connect_signaling(&mut self) -> Result<AppSignaling, AppClientError> {
        let r = self
            .grpc_client
            .build_request(
                "/proto.rpc.webrtc.v1.SignalingService/Answer",
                Some(&self.jwt),
                &self.config.rpc_host,
            )
            .map_err(AppClientError::AppOtherError)?;

        let (tx, rx) = self
            .grpc_client
            .send_request_bidi::<AnswerResponse, AnswerRequest>(r, None)
            .await
            .map_err(AppClientError::AppOtherError)?;
        Ok(AppSignaling(tx, rx))
    }

    // returns both a response from the robot config request and the timestamp of the response
    // taken from its header for the purposes of timestamping configuration logs and returning
    // `last_reconfigured` values for resource statuses.
    pub fn get_config(
        &mut self,
    ) -> Result<(Box<ConfigResponse>, Option<DateTime<FixedOffset>>), AppClientError> {
        let r = self
            .grpc_client
            .build_request("/viam.app.v1.RobotService/Config", Some(&self.jwt), "")
            .map_err(AppClientError::AppOtherError)?;

        let agent = AgentInfo {
            os: "esp32".to_string(),
            host: "esp32".to_string(),
            ips: vec![self.ip.to_string()],
            version: env!("CARGO_PKG_VERSION").to_string(),
            git_revision: "".to_string(),
            platform: Some("esp32".to_string()),
        };

        let req = ConfigRequest {
            agent_info: Some(agent),
            id: self.config.robot_id.clone(),
        };
        let body = encode_request(req)?;

        let (mut r, headers) = self.grpc_client.send_request(r, body)?;

        let datetime = if let Some(date_val) = headers.get("date") {
            let date_str = date_val
                .to_str()
                .map_err(|_| AppClientError::AppConfigHeaderValueParseError)?;
            DateTime::parse_from_rfc2822(date_str).ok()
        } else {
            None
        };

        let r = r.split_off(5);

        Ok((Box::new(ConfigResponse::decode(r)?), datetime))
    }

    pub fn push_logs(&mut self, logs: Vec<LogEntry>) -> Result<(), AppClientError> {
        let r = self
            .grpc_client
            .build_request("/viam.app.v1.RobotService/Log", Some(&self.jwt), "")
            .map_err(AppClientError::AppOtherError)?;

        let req = LogRequest {
            id: self.config.robot_id.clone(),
            logs,
        };

        let body = encode_request(req)?;

        self.grpc_client.send_request(r, body)?;

        Ok(())
    }
}

impl<'a> Drop for AppClient<'a> {
    fn drop(&mut self) {
        log::debug!("dropping AppClient")
    }
}
