#[allow(unused_imports)] // need this trait for `get_attribute`
use crate::common::config::Component;
use crate::common::config::DynamicComponentConfig;
use crate::proto::app::v1::ConfigResponse;
use embedded_svc::{
    http::{client::Client as HttpClient, Method},
    io::Write,
};
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
use log::*;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum WebhookError {
    #[error("`webhook` not present in board attributes")]
    NoWebhook,
    #[error("`webhook-secret` not present in board attributes")]
    NoWebhookSecret,
    #[error("Error with webhook request: {0}")]
    RequestError(String),
    #[error("Error in config: {0}")]
    ConfigError(String),
}

/// Checks the board config for a `webhook` and `webhook-secret` attribute, sending a GET
/// request with credentials for an SDK to make a connection.
pub fn handle_webhook(config: ConfigResponse) -> Result<(), WebhookError> {
    let config = config
        .config
        .as_ref()
        .ok_or_else(|| WebhookError::ConfigError("board does not have config".to_string()))?;
    let components = &config.components; // component config
    let cloud = config.cloud.as_ref().ok_or_else(|| {
        WebhookError::ConfigError("board config does not have cloud config".to_string())
    })?;
    let fqdn = &cloud.fqdn; // robot's url
    let board_cfg: DynamicComponentConfig = components
        .iter()
        .find(|x| x.r#type == "board")
        .ok_or_else(|| {
            WebhookError::ConfigError("board component not found in config".to_string())
        })?
        .try_into()
        .map_err(|_| {
            WebhookError::ConfigError(
                "could not convert board to DynamicComponentConfig".to_string(),
            )
        })?;

    let webhook = board_cfg
        .get_attribute::<String>("webhook")
        .map_err(|_| WebhookError::NoWebhook)?;
    let secret = board_cfg
        .get_attribute::<String>("webhook-secret")
        .map_err(|_| WebhookError::NoWebhookSecret)?;
    let payload = json!({
        "location": fqdn,
        "secret": secret,
        "board": board_cfg.name,
    })
    .to_string();

    let mut client = HttpClient::wrap(
        EspHttpConnection::new(&HttpConfiguration {
            crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
            ..Default::default()
        })
        .map_err(|e| WebhookError::RequestError(e.to_string()))?,
    );

    let payload = payload.as_bytes();
    let headers = [
        ("accept", "text/plain"),
        ("content-type", "application/json"),
        ("connection", "close"),
        ("content-length", &format!("{}", payload.len())),
    ];
    let mut request = client
        .request(Method::Get, &webhook, &headers)
        .map_err(|e| WebhookError::RequestError(e.to_string()))?;
    request
        .write_all(payload)
        .map_err(|_| WebhookError::RequestError("failed to write payload".to_string()))?;
    request
        .flush()
        .map_err(|e| WebhookError::RequestError(e.to_string()))?;
    debug!("-> GET {}", webhook);
    let response = request
        .submit()
        .map_err(|e| WebhookError::RequestError(e.to_string()))?;
    debug!("<- {}", response.status());
    Ok(())
}
