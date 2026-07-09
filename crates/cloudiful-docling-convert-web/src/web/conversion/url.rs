use axum::body::Bytes;
use cloudiful_docling_convert::{InputDocument, InputKind, PdfConvertError};
use reqwest::Response;

use super::super::state::{AppState, TaskConfig};
use super::file::process_file_conversion;
use super::support::{MAX_REMOTE_DOWNLOAD_BYTES, update_processing_status, validate_pdf_download};

pub async fn process_url_conversion(
    state: AppState,
    task_id: String,
    url: String,
    fallback_file_name: String,
    config: TaskConfig,
) -> Result<(), PdfConvertError> {
    update_processing_status(&state, &task_id, 10, "Downloading from URL...").await;

    let response = state
        .http_client
        .get(&url)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                PdfConvertError::validation_error(
                    "url",
                    "Download timed out. Please check the URL or try again.",
                )
            } else if e.is_connect() {
                PdfConvertError::validation_error(
                    "url",
                    "Failed to connect to URL. Please check the URL is correct.",
                )
            } else {
                PdfConvertError::validation_error("url", format!("Network error: {}", e))
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(PdfConvertError::validation_error(
            "url",
            format!(
                "HTTP error {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown error")
            ),
        ));
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let file_data = read_response_body_limited(response).await?;

    let mut file_name = if fallback_file_name.is_empty() {
        "downloaded".to_string()
    } else {
        fallback_file_name
    };
    let input_kind = InputKind::from_filename_and_media_type(&file_name, Some(&content_type))
        .ok_or_else(|| {
            PdfConvertError::validation_error(
                "url",
                format!(
                    "unsupported downloaded file type '{}', content-type '{}'",
                    file_name, content_type
                ),
            )
        })?;

    if std::path::Path::new(&file_name).extension().is_none() {
        file_name = format!(
            "{}.{}",
            file_name,
            input_kind.default_extension_for_media_type(Some(&content_type))
        );
    }

    update_processing_status(
        &state,
        &task_id,
        20,
        format!(
            "Downloaded {:?} (Content-Type: {})",
            input_kind, content_type
        ),
    )
    .await;

    validate_pdf_download(input_kind, &file_data)?;

    process_file_conversion(
        state,
        task_id,
        InputDocument::new(
            file_name.clone(),
            input_kind.canonical_media_type(&file_name, Some(&content_type)),
            file_data,
        ),
        config,
    )
    .await
}

async fn read_response_body_limited(mut response: Response) -> Result<Bytes, PdfConvertError> {
    if let Some(content_length) = response.content_length() {
        if content_length > MAX_REMOTE_DOWNLOAD_BYTES as u64 {
            return Err(PdfConvertError::validation_error(
                "url",
                format!(
                    "Downloaded file exceeds {} MB limit",
                    MAX_REMOTE_DOWNLOAD_BYTES / (1024 * 1024)
                ),
            ));
        }
    }

    let mut downloaded = 0usize;
    let mut body = Vec::new();
    if let Some(content_length) = response.content_length() {
        body.reserve((content_length as usize).min(MAX_REMOTE_DOWNLOAD_BYTES));
    }

    while let Some(chunk) = response.chunk().await.map_err(|e| {
        PdfConvertError::validation_error(
            "url",
            format!("Failed to read downloaded content: {}", e),
        )
    })? {
        downloaded += chunk.len();
        if downloaded > MAX_REMOTE_DOWNLOAD_BYTES {
            return Err(PdfConvertError::validation_error(
                "url",
                format!(
                    "Downloaded file exceeds {} MB limit",
                    MAX_REMOTE_DOWNLOAD_BYTES / (1024 * 1024)
                ),
            ));
        }
        body.extend_from_slice(&chunk);
    }

    Ok(Bytes::from(body))
}
