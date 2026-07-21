use log::{debug, warn};
use reqwest::{Client, Response, StatusCode};
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

use crate::error::{PdfConvertError, Result};

const MAX_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY_MS: u64 = 1000;
const MAX_RETRY_DELAY_MS: u64 = 30000;

pub async fn get_request(client: &Client, url: &str, context: &str) -> Result<Response> {
    get_request_with_conn_close(client, url, context, false).await
}

pub async fn get_request_with_conn_close(
    client: &Client,
    url: &str,
    context: &str,
    close_connection: bool,
) -> Result<Response> {
    let mut request = client.get(url);
    if close_connection {
        request = request.header(reqwest::header::CONNECTION, "close");
    }

    let response = request.send().await.map_err(PdfConvertError::from)?;

    handle_response(response, context).await
}

pub async fn handle_response(response: Response, context: &str) -> Result<Response> {
    let status = response.status();
    if !status.is_success() {
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
        let message = if response_text.trim().is_empty() {
            status_message
        } else {
            let details = extract_error_details(&response_text);
            if details == response_text {
                format!("{status_message}; response body: {response_text}")
            } else {
                format!("{status_message}; {details}; response body: {response_text}")
            }
        };
        return Err(PdfConvertError::api_error(Some(status_code), message));
    }
    Ok(response)
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
            log::debug!(
                "Retrying {} (attempt {}/{})",
                operation_name,
                attempt,
                MAX_RETRIES
            );
        }

        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    log::debug!(
                        "{} succeeded after {} attempts",
                        operation_name,
                        attempt + 1
                    );
                }
                return Ok(result);
            }
            Err(e) => {
                if !is_retriable_error_type(&e) {
                    debug!("Non-retriable error for {}: {}", operation_name, e);
                    return Err(e);
                }

                last_error = Some(e);

                if attempt < MAX_RETRIES {
                    let err_msg = last_error.as_ref().unwrap().to_string();
                    warn!(
                        "Transient error on {} (attempt {}/{}): {}. Retrying in {}ms...",
                        operation_name,
                        attempt + 1,
                        MAX_RETRIES + 1,
                        err_msg,
                        delay
                    );

                    sleep(Duration::from_millis(delay)).await;

                    delay = (delay * 2).min(MAX_RETRY_DELAY_MS);
                } else {
                    let err_msg = last_error.as_ref().unwrap().to_string();
                    warn!(
                        "All retries exhausted for {}. Last error: {}",
                        operation_name, err_msg
                    );
                }
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| PdfConvertError::api_error(None, "Unknown error after retries")))
}

pub fn is_retriable_error_type(error: &PdfConvertError) -> bool {
    match error {
        PdfConvertError::ApiError {
            status_code,
            message,
            ..
        } => {
            if let Some(code) = status_code {
                let status = StatusCode::from_u16(*code).ok();
                if let Some(s) = status {
                    return s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS;
                }
            }

            let msg_lower = message.to_lowercase();
            msg_lower.contains("transient")
                || msg_lower.contains("timeout")
                || msg_lower.contains("connection")
                || msg_lower.contains("network")
                || msg_lower.contains("closed")
                || msg_lower.contains("reset")
                || msg_lower.contains("broken pipe")
                || msg_lower.contains("eoferror")
                || msg_lower.contains("incomplete")
                || msg_lower.contains("end of file")
        }
        PdfConvertError::IoError { .. } => true,
        _ => false,
    }
}

pub fn extract_error_details(response_text: &str) -> String {
    match serde_json::from_str::<Value>(response_text) {
        Ok(json) => {
            let mut details = Vec::new();

            if let Some(error) = json.get("error") {
                details.push(format!("error: {}", format_json_value(error)));
            }
            if let Some(message) = json.get("message") {
                details.push(format!("message: {}", format_json_value(message)));
            }
            if let Some(detail) = json.get("detail") {
                details.push(format!("detail: {}", format_json_value(detail)));
            }

            if details.is_empty() {
                json.to_string()
            } else {
                details.join(", ")
            }
        }
        Err(_) => response_text.to_string(),
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
    use crate::error::PdfConvertError;

    #[test]
    fn test_is_retriable_error_type() {
        let server_error = PdfConvertError::api_error(Some(500), "Internal Server Error");
        assert!(is_retriable_error_type(&server_error));

        let bad_gateway = PdfConvertError::api_error(Some(502), "Bad Gateway");
        assert!(is_retriable_error_type(&bad_gateway));

        let too_many_requests = PdfConvertError::api_error(Some(429), "Too Many Requests");
        assert!(is_retriable_error_type(&too_many_requests));

        let bad_request = PdfConvertError::api_error(Some(400), "Bad Request");
        assert!(!is_retriable_error_type(&bad_request));

        let unauthorized = PdfConvertError::api_error(Some(401), "Unauthorized");
        assert!(!is_retriable_error_type(&unauthorized));

        let timeout_error = PdfConvertError::api_error(None, "Operation timeout occurred");
        assert!(is_retriable_error_type(&timeout_error));

        let conn_reset = PdfConvertError::api_error(None, "Connection reset by peer");
        assert!(is_retriable_error_type(&conn_reset));

        let broken_pipe = PdfConvertError::api_error(None, "Broken pipe (os error 32)");
        assert!(is_retriable_error_type(&broken_pipe));

        let io_err = PdfConvertError::io_error("test", std::io::Error::other("io error"));
        assert!(is_retriable_error_type(&io_err));

        let val_err = PdfConvertError::validation_error("param", "reason");
        assert!(!is_retriable_error_type(&val_err));
    }

    #[test]
    fn test_extract_error_details() {
        let json_error = r#"{"error": "bad_request", "message": "Invalid parameter", "detail": "page_range must be positive"}"#;
        let details = extract_error_details(json_error);
        assert!(details.contains("error: bad_request"));
        assert!(details.contains("message: Invalid parameter"));
        assert!(details.contains("detail: page_range must be positive"));

        let simple_json = r#"{"msg":"unknown format"}"#;
        let details = extract_error_details(simple_json);
        assert_eq!(details, simple_json);

        let non_json = "Internal Server Error";
        let details = extract_error_details(non_json);
        assert_eq!(details, non_json);

        let structured_json = r#"{"detail":[{"loc":["body","files"],"msg":"field required"}]}"#;
        let details = extract_error_details(structured_json);
        assert!(details.contains("detail: [{"));
        assert!(details.contains("field required"));

        let long_text = "x".repeat(256);
        assert_eq!(extract_error_details(&long_text), long_text);
    }
}
