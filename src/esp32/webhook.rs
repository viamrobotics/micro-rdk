#[allow(unused_imports)] // need this trait for `get_attribute`
use crate::common::config::{AttributeError, Component, DynamicComponentConfig};
use crate::proto::app::v1::RobotConfig;
use embedded_svc::{
    http::{
        client::{Client as HttpClient, Connection},
        Method,
    },
    io::Write,
};
use log::*;
use serde_json::json;
use thiserror::Error;
use url::{ParseError, Url};

#[derive(Debug)]
pub enum WebhookResult {
    Success,
    NoWebhook,
    NoBoardConfigured,
}

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("invalid webhook url: {0}")]
    InvalidEndpoint(ParseError),
    #[error("{0}")]
    AttributeError(AttributeError),
    #[error("error in config: {0}")]
    ConfigError(String),
    #[error("{0}")]
    RequestError(String),
    #[error("did not receive successful response")]
    Other(String),
}

#[derive(Debug)]
pub struct Webhook {
    /// Fully qualified domain name (or URL) of the robot
    fqdn: String,
    /// Endpoint that will be sent a GET request with credential information
    endpoint: Option<String>,
    /// Location Secret used by the webhook's SDK script to connect
    secret: Option<String>,
    num_retries: u8,
}

impl Default for Webhook {
    fn default() -> Self {
        Self {
            fqdn: String::default(),
            endpoint: None,
            secret: None,
            num_retries: 3,
        }
    }
}

impl Webhook {
    pub fn from_robot_config(config: &RobotConfig) -> Result<Self, WebhookError> {
        let components = &config.components;
        let cloud = config.cloud.as_ref().ok_or_else(|| {
            WebhookError::ConfigError("robot config does not have cloud config".to_string())
        })?;
        let mut webhook = Self {
            fqdn: cloud.fqdn.clone(),
            ..Default::default()
        };

        let board_cfg: DynamicComponentConfig = {
            let board = components.iter().find(|x| x.r#type == "board");
            if board.is_none() {
                // return empty webhook
                return Ok(webhook);
            }

            board
                .unwrap()
                .try_into()
                .map_err(WebhookError::AttributeError)?
        };

        webhook.endpoint = {
            if let Ok(url) = board_cfg.get_attribute::<String>("webhook") {
                Some(
                    Url::parse(&url)
                        .map_err(WebhookError::InvalidEndpoint)?
                        .into(),
                )
            } else {
                return Ok(webhook);
            }
        };

        webhook.secret = board_cfg
            .get_attribute::<String>("webhook-secret")
            .map_err(WebhookError::AttributeError)
            .ok();

        Ok(webhook)
    }
    pub fn with_retries(self, num_retries: u8) -> Self {
        if num_retries == 0 {
            return self;
        }

        let Self {
            fqdn,
            endpoint,
            secret,
            num_retries: _,
        } = self;

        Self {
            fqdn,
            endpoint,
            secret,
            num_retries,
        }
    }
    pub fn has_endpoint(&self) -> bool {
        self.endpoint.is_some()
    }
    pub fn payload(&self) -> String {
        json!({
            "location": self.fqdn,
            "secret": self.secret,
        })
        .to_string()
    }
    pub fn send<C: Connection>(&self, client: &mut HttpClient<C>) -> Result<(), WebhookError> {
        for _ in 0..self.num_retries {
            match self.send_request(client) {
                Ok(_) => {
                    debug!("webhook request succeeded");
                    break;
                }
                Err(_) => {
                    error!("webhook request failed");
                    continue;
                }
            }
        }
        Ok(())
    }
    pub fn send_request<'a, C: Connection>(
        &'a self,
        client: &'a mut HttpClient<C>,
    ) -> Result<(), WebhookError> {
        // fails silently if webhook isn't configured
        if self.endpoint.is_none() {
            return Ok(());
        }

        let payload = self.payload();
        let payload = payload.as_bytes();

        let headers = [
            ("accept", "text/plain"),
            ("content-type", "application/json"),
            ("connection", "close"),
            ("content-length", &format!("{}", payload.len())),
        ];

        // safe to unwrap from earlier endpoint check
        let url = self.endpoint.clone().unwrap();
        let mut request = client
            .request(Method::Get, &url, &headers)
            .map_err(|e| WebhookError::RequestError(format!("{e:?}")))?;

        request
            .write_all(payload)
            .map_err(|e| WebhookError::RequestError(format!("{e:?}")))?;

        request
            .flush()
            .map_err(|e| WebhookError::RequestError(format!("{e:?}")))?;

        request
            .submit()
            .map_err(|e| WebhookError::RequestError(format!("{e:?}")))?;
        Ok(())
    }
}
