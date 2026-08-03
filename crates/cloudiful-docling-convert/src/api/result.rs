use bytes::Bytes;
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

pub(crate) async fn parse_response(response: Response, context: &str) -> Result<DoclingResult> {
    let response = handle_response(response, context).await?;
    let is_zip = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("application/zip"));
    let bytes = response.bytes().await.map_err(PdfConvertError::from)?;
    parse_bytes(bytes, is_zip)
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
