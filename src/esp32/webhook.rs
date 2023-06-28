#[allow(unused_imports)] // need this trait for `get_attribute`
use crate::common::config::Component;
use crate::common::config::DynamicComponentConfig;
use crate::proto::app::v1::RobotConfig;
use embedded_svc::{
    http::{
        client::{Client as HttpClient, Connection},
        Method,
    },
    io::Write,
};
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
use log::*;
use serde_json::json;
use thiserror::Error;

/// Checks the board config for a `webhook` and `webhook-secret` attribute, sending a GET
/// request with credentials for an SDK to make a connection.
pub fn handle_webhook(config: RobotConfig) -> Result<(), WebhookError> {
    // get Webhook Struct
    let webhook = Webhook::from_robot_config(config)?;

    let mut client = HttpClient::wrap(
        EspHttpConnection::new(&HttpConfiguration {
            crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
            ..Default::default()
        })
        .map_err(|e| WebhookError::Other(e.to_string()))?,
    );

    webhook.send_request(&mut client)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("`webhook` not present in board attributes")]
    NoWebhook,
    #[error("`webhook-secret` not present in board attributes")]
    NoWebhookSecret,
    #[error("error in config: {0}")]
    ConfigError(String),
    #[error("failed to send request")]
    RequestError,
    #[error("did not receive successful response")]
    ResponseError,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Default)]
pub struct Webhook {
    /// Fully qualified domain name (or URL) of the robot
    fqdn: String,
    /// Location Secret used by the webhook's SDK script to connect
    secret: String,
    /// Endpoint that will be sent a GET request with credential information
    endpoint: String,
}

impl Webhook {
    pub fn from_robot_config(config: RobotConfig) -> Result<Self, WebhookError> {
        let components = config.components;
        let cloud = config.cloud.as_ref().ok_or_else(|| {
            WebhookError::ConfigError("robot config does not have cloud config".to_string())
        })?;
        let fqdn = cloud.fqdn.clone();
        let board_cfg: DynamicComponentConfig = components
            .iter()
            .find(|x| x.r#type == "board")
            .ok_or_else(|| {
                WebhookError::ConfigError("board component not found in robot config".to_string())
            })?
            .try_into()
            .map_err(|_| {
                WebhookError::ConfigError(
                    "could not convert board config to DynamicComponentConfig".to_string(),
                )
            })?;

        let endpoint = board_cfg
            .get_attribute::<String>("webhook")
            .map_err(|_| WebhookError::NoWebhook)?;
        let secret = board_cfg
            .get_attribute::<String>("webhook-secret")
            .map_err(|_| WebhookError::NoWebhookSecret)?;
        Ok(Self {
            fqdn,
            endpoint,
            secret,
        })
    }
    pub fn payload(&self) -> String {
        json!({
            "location": self.fqdn,
            "secret": self.secret,
        })
        .to_string()
    }
    pub fn send_request<'a, C: Connection>(
        &'a self,
        client: &'a mut HttpClient<C>,
    ) -> Result<(), WebhookError> {
        let payload = self.payload();
        let payload = payload.as_bytes();

        let headers = [
            ("accept", "text/plain"),
            ("content-type", "application/json"),
            ("connection", "close"),
            ("content-length", &format!("{}", payload.len())),
        ];

        let mut request = client
            .request(Method::Get, &self.endpoint, &headers)
            .map_err(|_| WebhookError::RequestError)?;

        request
            .write_all(payload)
            .map_err(|_| WebhookError::RequestError)?;

        request.flush().map_err(|_| WebhookError::RequestError)?;

        request.submit().map_err(|_| WebhookError::ResponseError)?;
        Ok(())
    }
}
