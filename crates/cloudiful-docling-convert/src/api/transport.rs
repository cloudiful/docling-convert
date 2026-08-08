use log::{debug, warn};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

use crate::error::{PdfConvertError, Result};

use super::docling::DoclingConfig;

const MAX_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY_MS: u64 = 1000;
const MAX_RETRY_DELAY_MS: u64 = 30000;

#[derive(Clone)]
pub(crate) struct Transport {
    pub client: Client,
    config: DoclingConfig,
}

impl Transport {
    pub fn new(config: DoclingConfig) -> Result<Self> {
        let timeout = config
            .request_timeout
            .unwrap_or_else(default_request_timeout);
        let client = Client::builder()
            .timeout(timeout)
            .tcp_keepalive(Duration::from_secs(60))
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| PdfConvertError::api_error(None, error.to_string()))?;

        Ok(Self { client, config })
    }

    pub fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.config.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    pub fn request(&self, method: reqwest::Method, path: &str) -> RequestBuilder {
        let mut request = self.client.request(method, self.url(path));
        if let Some(api_key) = self.config.api_key.as_deref() {
            request = request.header("X-Api-Key", api_key);
        }
        if let Some(tenant_id) = self.config.tenant_id.as_deref() {
            request = request.header("X-Tenant-Id", tenant_id);
        }
        request
    }

    pub fn config(&self) -> &DoclingConfig {
        &self.config
    }
}

pub fn default_request_timeout() -> Duration {
    let seconds = std::env::var("DOCLING_HTTP_TIMEOUT_SECS")
        .ok()
        .or_else(|| std::env::var("DOCLING_SERVE_MAX_SYNC_WAIT").ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3600)
        .max(1);
    Duration::from_secs(seconds)
}

pub fn default_task_timeout() -> Duration {
    let seconds = std::env::var("DOCLING_TASK_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3600)
        .max(1);
    Duration::from_secs(seconds)
}

pub async fn handle_response(response: Response, context: &str) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let status_code = status.as_u16();
    let status_message = format!(
        "{} failed: {} {}",
        context,
        status,
        status.canonical_reason().unwrap_or("Unknown")
    );
    let response_text = response.text().await.map_err(|error| {
        PdfConvertError::api_error(
            Some(status_code),
            format!("{status_message}; failed to read response body: {error}"),
        )
    })?;
    let details = extract_error_details(&response_text);
    let message = if response_text.trim().is_empty() {
        status_message
    } else if details == response_text {
        format!("{status_message}; response body: {response_text}")
    } else {
        format!("{status_message}; {details}; response body: {response_text}")
    };

    Err(PdfConvertError::api_error(Some(status_code), message))
}

pub async fn retry_with_backoff<F, Fut, T>(operation: F, operation_name: &str) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;
    let mut delay = INITIAL_RETRY_DELAY_MS;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            debug!(
                "Retrying {} (attempt {}/{})",
                operation_name, attempt, MAX_RETRIES
            );
        }

        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) if !is_retriable_error_type(&error) => return Err(error),
            Err(error) => {
                if attempt == MAX_RETRIES {
                    return Err(error);
                }
                warn!(
                    "Transient error on {} (attempt {}/{}): {}. Retrying in {}ms...",
                    operation_name,
                    attempt + 1,
                    MAX_RETRIES + 1,
                    error,
                    delay
                );
                last_error = Some(error);
                sleep(Duration::from_millis(delay)).await;
                delay = (delay * 2).min(MAX_RETRY_DELAY_MS);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| PdfConvertError::api_error(None, "unknown transport error")))
}

pub fn is_retriable_error_type(error: &PdfConvertError) -> bool {
    match error {
        PdfConvertError::ApiError {
            status_code,
            message,
            ..
        } => {
            if let Some(code) = status_code {
                if let Ok(status) = StatusCode::from_u16(*code) {
                    return status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS;
                }
            }
            let message = message.to_ascii_lowercase();
            [
                "timeout",
                "connection",
                "network",
                "closed",
                "reset",
                "broken pipe",
                "eof",
                "incomplete",
            ]
            .iter()
            .any(|term| message.contains(term))
        }
        PdfConvertError::IoError { .. } => true,
        _ => false,
    }
}

pub fn extract_error_details(response_text: &str) -> String {
    let Ok(json) = serde_json::from_str::<Value>(response_text) else {
        return response_text.to_string();
    };

    let mut details = Vec::new();
    collect_error_details(&json, &mut details);
    if details.is_empty() {
        json.to_string()
    } else {
        details.join(", ")
    }
}

fn collect_error_details(value: &Value, details: &mut Vec<String>) {
    let Some(object) = value.as_object() else {
        return;
    };

    if let Some(failure) = object.get("failure").and_then(Value::as_object) {
        if let Some(message) = failure.get("message") {
            details.push(format!("failure.message: {}", format_json_value(message)));
        }
    }
    for key in ["error_message", "message", "error", "detail"] {
        if let Some(value) = object.get(key) {
            details.push(format!("{key}: {}", format_json_value(value)));
        }
    }

    if let Some(errors) = object.get("errors").and_then(Value::as_array) {
        for error in errors {
            if let Some(message) = error.get("error_message").or_else(|| error.get("message")) {
                details.push(format!("document error: {}", format_json_value(message)));
            } else {
                details.push(format!("document error: {}", format_json_value(error)));
            }
        }
    }
}

fn format_json_value(value: &Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_and_document_errors() {
        let details = extract_error_details(
            r#"{"failure":{"message":"failed"},"error_message":"outer","errors":[{"error_message":"page failed"}],"detail":"bad request"}"#,
        );
        assert!(details.contains("failure.message: failed"));
        assert!(details.contains("document error: page failed"));
        assert!(details.contains("detail: bad request"));
    }
}
