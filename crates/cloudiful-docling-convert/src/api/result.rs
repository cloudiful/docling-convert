use reqwest::Client;
use serde_json::Value;

use crate::error::{PdfConvertError, Result};

use super::client::{get_request_with_conn_close, retry_with_backoff};

pub async fn get_task_result(
    client: &Client,
    docling_base_url: &str,
    task_id: &str,
    use_new_conn: bool,
) -> Result<Value> {
    let operation = || async {
        let url = format!("{}/result/{}", docling_base_url, task_id);
        let response =
            get_request_with_conn_close(client, &url, "Fetching task result", use_new_conn).await?;

        let result = response
            .json::<Value>()
            .await
            .map_err(|e| PdfConvertError::parse_error("task result response", e.to_string()))?;

        Ok(result)
    };

    retry_with_backoff(operation, &format!("get_task_result({})", task_id)).await
}
