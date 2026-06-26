use reqwest::Client;

use crate::error::{PdfConvertError, Result};
use crate::models::TaskStatusResponse;

use super::client::{extract_error_details, get_request, retry_with_backoff};

pub async fn check_task_status(
    client: &Client,
    docling_base_url: &str,
    task_id: &str,
) -> Result<bool> {
    let operation =
        || async { check_task_status_internal(client, docling_base_url, task_id).await };

    retry_with_backoff(operation, &format!("check_task_status({})", task_id)).await
}

async fn check_task_status_internal(
    client: &Client,
    docling_base_url: &str,
    task_id: &str,
) -> Result<bool> {
    let url = format!("{}/status/poll/{}", docling_base_url, task_id);
    let response = get_request(client, &url, "Polling task status").await?;

    let response_text = response.text().await?;

    match serde_json::from_str::<TaskStatusResponse>(&response_text) {
        Ok(status_response) => {
            let status = status_response.task_status.as_str();
            match status {
                "success" => Ok(true),
                "failure" | "revoked" => {
                    let error_details = extract_error_details(&response_text);
                    Err(PdfConvertError::api_task_failed(status, error_details))
                }
                _ => Ok(false),
            }
        }
        Err(_) => Err(PdfConvertError::parse_error(
            "task status response",
            format!(
                "Task {} returned invalid response: {}",
                task_id, response_text
            ),
        )),
    }
}
