#![allow(unused)]
use bytes::{BufMut, Bytes, BytesMut};
use futures_lite::Future;
use prost::{DecodeError, EncodeError, Message};
use std::{net::Ipv4Addr, pin::Pin, rc::Rc};
use thiserror::Error;

use crate::proto::{
    app::v1::{AgentInfo, ConfigRequest, ConfigResponse},
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
}

#[derive(Debug)]
pub struct AppClientConfig {
    robot_id: String,
    robot_secret: String,
    ip: Ipv4Addr,
}

impl Default for AppClientConfig {
    fn default() -> Self {
        Self {
            robot_id: "".to_owned(),
            robot_secret: "".to_owned(),
            ip: Ipv4Addr::new(0, 0, 0, 0),
        }
    }
}

impl AppClientConfig {
    pub fn new(robot_secret: String, robot_id: String, ip: Ipv4Addr) -> Self {
        AppClientConfig {
            robot_id,
            robot_secret,
            ip,
        }
    }
}

pub struct AppClientBuilder<'a> {
    grpc_client: GrpcClient<'a>,
    config: AppClientConfig,
}

fn encode_request<T>(req: T) -> Result<Bytes, AppClientError>
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
    pub fn new(grpc_client: GrpcClient<'a>, config: AppClientConfig) -> Self {
        Self {
            grpc_client,
            config,
        }
    }
    /// Consume the AppClientBuilder and returns an AppClient. This function will panic if
    /// the received config doesn't contain an fqdn field.
    pub fn build(mut self) -> Result<AppClient<'a>, AppClientError> {
        let jwt = self.get_jwt_token()?;

        let config = self.read_config(&jwt)?;

        let rpc_host = config
            .config
            .as_ref()
            .unwrap()
            .cloud
            .as_ref()
            .unwrap()
            .fqdn
            .clone();

        Ok(AppClient {
            grpc_client: self.grpc_client,
            jwt,
            robot_config: config,
            rpc_host,
            ip: self.config.ip,
        })
    }
    fn get_jwt_token(&mut self) -> Result<String, AppClientError> {
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
            .map_err(AppClientError::AppOtherError)?;
        let r = r.split_off(5);
        let r = AuthenticateResponse::decode(r).map_err(AppClientError::AppDecodeError)?;
        Ok(format!("Bearer {}", r.access_token))
    }

    fn read_config(&mut self, jwt: &str) -> Result<Box<ConfigResponse>, AppClientError> {
        let r = self
            .grpc_client
            .build_request("/viam.app.v1.RobotService/Config", Some(jwt), "")
            .map_err(AppClientError::AppOtherError)?;

        let agent = AgentInfo {
            os: "esp32".to_string(),
            host: "esp32".to_string(),
            ips: vec![self.config.ip.to_string()],
            version: "0.0.2".to_string(),
            git_revision: "".to_string(),
        };

        let req = ConfigRequest {
            agent_info: Some(agent),
            id: self.config.robot_id.clone(),
        };
        let body = encode_request(req)?;

        let mut r = self.grpc_client.send_request(r, body)?;
        let r = r.split_off(5);

        Ok(Box::new(ConfigResponse::decode(r)?))
    }
}

pub struct AppClient<'a> {
    // Potentially consider leak to make it a static reference for the lifetime of the program?
    robot_config: Box<ConfigResponse>,
    jwt: String,
    grpc_client: GrpcClient<'a>,
    rpc_host: String,
    ip: Ipv4Addr,
}

pub(crate) struct AppSignaling(
    pub(crate) GrpcMessageSender<AnswerResponse>,
    pub(crate) GrpcMessageStream<AnswerRequest>,
);

impl<'a> AppClient<'a> {
    pub(crate) fn connect_signaling(&mut self) -> Result<AppSignaling, AppClientError> {
        let r = self
            .grpc_client
            .build_request(
                "/proto.rpc.webrtc.v1.SignalingService/Answer",
                Some(&self.jwt),
                &self.rpc_host,
            )
            .map_err(AppClientError::AppOtherError)?;

        let (tx, rx) = self
            .grpc_client
            .send_request_bidi::<AnswerResponse, AnswerRequest>(r, None)
            .map_err(AppClientError::AppOtherError)?;
        Ok(AppSignaling(tx, rx))
    }
    pub(crate) fn get_config(self) -> Box<ConfigResponse> {
        self.robot_config
    }
    pub fn connect_webrtc<E, D, C>(
        &mut self,
        cert: Rc<C>,
        exec: E,
        dtls: D,
    ) -> Result<WebRtcApi<'a, C, D, E>, AppClientError>
    where
        E: WebRtcExecutor<Pin<Box<dyn Future<Output = ()> + Send>>> + Clone + 'static,
        D: DtlsConnector,
        C: Certificate,
    {
        let signaling = self.connect_signaling()?;

        let cloned_exec = exec.clone();

        let mut webrtc = WebRtcApi::new(exec, signaling.0, signaling.1, cert, self.ip, dtls);

        cloned_exec
            .block_on(async { webrtc.answer().await })
            .map_err(AppClientError::AppWebRtcError)?;

        cloned_exec
            .block_on(async { webrtc.run_ice_until_connected().await })
            .map_err(AppClientError::AppWebRtcError)?;

        Ok(webrtc)
    }
}
