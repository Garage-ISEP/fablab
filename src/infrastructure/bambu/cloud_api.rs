//! Bambu cloud HTTP API (reverse-engineered, documented by OpenBambuAPI).

use reqwest::StatusCode;
use serde::Deserialize;

use crate::domain::errors::DomainError;
use crate::domain::filament::PrinterDevice;
use crate::domain::repositories::BambuCloudApi;

const API_BASE: &str = "https://api.bambulab.com";

pub struct ReqwestBambuCloudApi {
    http: reqwest::Client,
}

impl ReqwestBambuCloudApi {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { http }
    }

    async fn get(&self, path: &str, access_token: &str) -> Result<Vec<u8>, DomainError> {
        let response = self
            .http
            .get(format!("{API_BASE}{path}"))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| DomainError::BambuUnavailable(format!("request failed: {e}")))?;

        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(DomainError::BambuTokenRejected);
            }
            status if !status.is_success() => {
                return Err(DomainError::BambuUnavailable(format!("HTTP {status}")));
            }
            _ => {}
        }

        response
            .bytes()
            .await
            .map(|body| body.to_vec())
            .map_err(|e| DomainError::BambuUnavailable(format!("response read error: {e}")))
    }
}

impl Default for ReqwestBambuCloudApi {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct PreferenceResponse {
    uid: serde_json::Value,
}

#[derive(Deserialize)]
struct BindResponse {
    devices: Option<Vec<BoundDevice>>,
}

#[derive(Deserialize)]
struct BoundDevice {
    dev_id: String,
    name: Option<String>,
    dev_product_name: Option<String>,
}

#[async_trait::async_trait]
impl BambuCloudApi for ReqwestBambuCloudApi {
    async fn fetch_user_id(&self, access_token: &str) -> Result<String, DomainError> {
        let body = self
            .get("/v1/design-user-service/my/preference", access_token)
            .await?;
        let preference: PreferenceResponse = serde_json::from_slice(&body)
            .map_err(|e| DomainError::BambuUnavailable(format!("unexpected response: {e}")))?;

        // The uid is a number today; accept a string in case that changes.
        match preference.uid {
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::String(s) if !s.is_empty() => Ok(s),
            _ => Err(DomainError::BambuUnavailable("missing uid".to_owned())),
        }
    }

    async fn fetch_devices(&self, access_token: &str) -> Result<Vec<PrinterDevice>, DomainError> {
        let body = self
            .get("/v1/iot-service/api/user/bind", access_token)
            .await?;
        let bind: BindResponse = serde_json::from_slice(&body)
            .map_err(|e| DomainError::BambuUnavailable(format!("unexpected response: {e}")))?;

        Ok(bind
            .devices
            .unwrap_or_default()
            .into_iter()
            .map(|device| {
                let model = device.dev_product_name.unwrap_or_default();
                PrinterDevice {
                    name: device
                        .name
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| model.clone()),
                    serial: device.dev_id,
                    model,
                }
            })
            .collect())
    }
}
