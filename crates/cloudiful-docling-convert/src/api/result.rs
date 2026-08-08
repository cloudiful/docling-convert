use bytes::{Bytes, BytesMut};
use futures::{Stream, StreamExt};
use reqwest::Response;
use serde_json::Value;

use crate::error::{PdfConvertError, Result};
use crate::models::{
    ChunkDocumentResponse, ConvertDocumentResponse, DoclingErrorItem, TaskFailureResult,
};

use super::transport::handle_response;

#[derive(Debug, Clone)]
pub enum DoclingResult {
    Convert(ConvertDocumentResponse),
    Chunk(ChunkDocumentResponse),
    Failure(TaskFailureResult),
    Json(Value),
    Zip(Bytes),
}

#[derive(Debug, Clone)]
pub struct DoclingTaskResult {
    pub status: crate::models::ConversionStatus,
    pub result: DoclingResult,
    pub errors: Vec<String>,
}

impl DoclingResult {
    pub fn errors(&self) -> Vec<String> {
        match self {
            Self::Convert(response) => response.errors.iter().map(format_error).collect(),
            Self::Chunk(response) => response
                .documents
                .iter()
                .flat_map(|document| document.errors.iter())
                .map(format_error)
                .collect(),
            Self::Failure(response) => vec![response.failure.message.clone()],
            Self::Json(value) => collect_json_errors(value),
            Self::Zip(_) => Vec::new(),
        }
    }

    pub fn filename(&self) -> Option<&str> {
        match self {
            Self::Convert(response) => Some(&response.document.filename),
            Self::Chunk(response) => response
                .chunks
                .first()
                .map(|chunk| chunk.filename.as_str())
                .or_else(|| {
                    response
                        .documents
                        .first()
                        .map(|document| document.document.filename.as_str())
                }),
            Self::Failure(_) | Self::Json(_) | Self::Zip(_) => None,
        }
    }

    pub fn into_json(self) -> Option<Value> {
        match self {
            Self::Convert(response) => serde_json::to_value(response).ok(),
            Self::Chunk(response) => serde_json::to_value(response).ok(),
            Self::Failure(response) => serde_json::to_value(response).ok(),
            Self::Json(value) => Some(value),
            Self::Zip(_) => None,
        }
    }
}

pub(crate) async fn parse_response(
    response: Response,
    context: &str,
    body_limit: Option<usize>,
) -> Result<DoclingResult> {
    let response = handle_response(response, context).await?;
    let is_zip = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("application/zip"));
    let content_length = response.content_length();
    let bytes = read_body(response.bytes_stream(), content_length, body_limit, context).await?;
    parse_bytes(bytes, is_zip)
}

async fn read_body<S>(
    mut stream: S,
    content_length: Option<u64>,
    body_limit: Option<usize>,
    context: &str,
) -> Result<Bytes>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Unpin,
{
    if let Some(limit) = body_limit
        && content_length.is_some_and(|length| length > limit as u64)
    {
        return Err(result_too_large(context, limit, content_length));
    }

    let capacity = body_limit
        .zip(content_length)
        .and_then(|(limit, length)| {
            usize::try_from(length)
                .ok()
                .filter(|length| *length <= limit)
        })
        .unwrap_or_default();
    let mut body = BytesMut::with_capacity(capacity);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(PdfConvertError::from)?;
        let next_length = body.len().saturating_add(chunk.len());
        if let Some(limit) = body_limit
            && next_length > limit
        {
            return Err(result_too_large(
                context,
                limit,
                u64::try_from(next_length).ok(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body.freeze())
}

fn result_too_large(context: &str, limit: usize, actual: Option<u64>) -> PdfConvertError {
    let actual = actual
        .map(|length| format!("; response is at least {length} bytes"))
        .unwrap_or_default();
    PdfConvertError::operation_error(
        format!("reading {context} response"),
        format!("result body exceeds maximum of {limit} bytes{actual}"),
    )
}

pub(crate) fn parse_bytes(bytes: Bytes, is_zip: bool) -> Result<DoclingResult> {
    if is_zip || bytes.as_ref().starts_with(b"PK\x03\x04") {
        return Ok(DoclingResult::Zip(bytes));
    }

    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        PdfConvertError::parse_error("Docling result response", error.to_string())
    })?;
    parse_json(value)
}

pub(crate) fn parse_json(value: Value) -> Result<DoclingResult> {
    if value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "TaskFailureResult")
        || value.get("failure").is_some()
    {
        return serde_json::from_value(value)
            .map(DoclingResult::Failure)
            .map_err(|error| {
                PdfConvertError::parse_error("Docling failure result", error.to_string())
            });
    }

    if value.get("chunks").is_some() {
        return serde_json::from_value(value)
            .map(DoclingResult::Chunk)
            .map_err(|error| {
                PdfConvertError::parse_error("Docling chunk result", error.to_string())
            });
    }

    if value.get("document").is_some() && value.get("status").is_some() {
        return serde_json::from_value(value)
            .map(DoclingResult::Convert)
            .map_err(|error| {
                PdfConvertError::parse_error("Docling conversion result", error.to_string())
            });
    }

    Ok(DoclingResult::Json(value))
}

fn format_error(error: &DoclingErrorItem) -> String {
    let message = error
        .error_message
        .as_deref()
        .unwrap_or("unknown document error");
    match error.page_no {
        Some(page) => format!("page {page}: {message}"),
        None => message.to_string(),
    }
}

fn collect_json_errors(value: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    for key in ["error_message", "message"] {
        if let Some(message) = value.get(key).and_then(Value::as_str) {
            errors.push(message.to_string());
        }
    }
    if let Some(message) = value
        .get("failure")
        .and_then(|failure| failure.get("message"))
        .and_then(Value::as_str)
    {
        errors.push(message.to_string());
    }
    if let Some(message) = value.get("detail").and_then(Value::as_str) {
        errors.push(message.to_string());
    }
    if let Some(items) = value.get("errors").and_then(Value::as_array) {
        errors.extend(items.iter().filter_map(|item| {
            item.get("error_message")
                .or_else(|| item.get("message"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }));
    }
    errors
}

#[cfg(test)]
mod tests {
    use futures::stream;

    use super::*;

    #[tokio::test]
    async fn limited_reader_accepts_exact_limit() {
        let chunks = stream::iter([Ok::<_, reqwest::Error>(Bytes::from_static(b"1234"))]);

        let body = read_body(chunks, None, Some(4), "test").await.unwrap();

        assert_eq!(body, Bytes::from_static(b"1234"));
    }

    #[tokio::test]
    async fn limited_reader_rejects_chunked_body_over_limit() {
        let chunks = stream::iter([
            Ok::<_, reqwest::Error>(Bytes::from_static(b"1234")),
            Ok::<_, reqwest::Error>(Bytes::from_static(b"5")),
        ]);

        let error = read_body(chunks, None, Some(4), "test")
            .await
            .expect_err("oversized body should fail");

        assert!(error.to_string().contains("exceeds maximum of 4 bytes"));
    }

    #[tokio::test]
    async fn limited_reader_rejects_known_length_before_reading() {
        let chunks = stream::empty::<std::result::Result<Bytes, reqwest::Error>>();

        let error = read_body(chunks, Some(5), Some(4), "test")
            .await
            .expect_err("oversized content length should fail");

        assert!(error.to_string().contains("response is at least 5 bytes"));
    }
}
