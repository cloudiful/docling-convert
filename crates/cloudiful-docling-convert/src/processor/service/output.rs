use std::path::{Path, PathBuf};

use serde_json::Value;
use tokio::fs;

use super::DocumentConverter;
use crate::api::DoclingResult;
use crate::document::{
    ConvertedDocument, ConvertedDocumentMetadata, InputDocument, InputKind, OutputFormat,
};
use crate::error::{PdfConvertError, Result};
use crate::models::{ChunkDocumentResponse, ExportDocumentResponse};

impl DocumentConverter {
    pub fn calculate_output_path(
        output_dir: &Path,
        filename: &str,
        output_format: OutputFormat,
    ) -> PathBuf {
        let stem = Path::new(filename)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output");
        output_dir.join(format!("{}.{}", stem, output_format.extension()))
    }

    pub(super) async fn write_output_file(
        output_path: &Path,
        document: &ConvertedDocument,
        output_format: OutputFormat,
    ) -> Result<()> {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).await.map_err(|error| {
                PdfConvertError::io_error(
                    format!("creating output directory: {}", parent.display()),
                    error,
                )
            })?;
        }

        let content = select_output_content(document, output_format)?;
        fs::write(output_path, content).await.map_err(|error| {
            PdfConvertError::io_error(
                format!("writing output file: {}", output_path.display()),
                error,
            )
        })?;
        Ok(())
    }

    pub(super) fn build_document(
        input: &InputDocument,
        input_kind: InputKind,
        result: DoclingResult,
    ) -> ConvertedDocument {
        let mut document = empty_document(input, input_kind);
        match result {
            DoclingResult::Convert(response) => {
                document.filename = response.document.filename.clone();
                apply_export_document(&mut document, response.document);
                document.errors = response
                    .errors
                    .iter()
                    .filter_map(|error| error.error_message.clone())
                    .collect();
            }
            DoclingResult::Chunk(response) => {
                apply_chunk_response(&mut document, response);
            }
            DoclingResult::Json(value) => {
                apply_json_result(&mut document, value);
            }
            DoclingResult::Zip(bytes) => {
                document.archive = Some(bytes.to_vec());
            }
            DoclingResult::Failure(failure) => {
                document.errors.push(failure.failure.message);
            }
        }
        document
    }
}

fn empty_document(input: &InputDocument, input_kind: InputKind) -> ConvertedDocument {
    ConvertedDocument {
        filename: input.filename.clone(),
        markdown: None,
        text: None,
        json: None,
        html: None,
        doctags: None,
        doclang: None,
        chunks: Vec::new(),
        chunk_response: None,
        archive: None,
        metadata: ConvertedDocumentMetadata {
            input_kind,
            media_type: input.media_type.clone(),
        },
        errors: Vec::new(),
    }
}

fn apply_export_document(document: &mut ConvertedDocument, response: ExportDocumentResponse) {
    document.markdown = response.md_content.clone();
    document.text = response.text_content.clone();
    document.json = response.json_content.clone();
    document.html = response.html_content.clone();
    document.doctags = response.doctags_content.clone();
    document.doclang = response.doclang_content.clone();
}

fn apply_chunk_response(document: &mut ConvertedDocument, response: ChunkDocumentResponse) {
    document.filename = response
        .chunks
        .first()
        .map(|chunk| chunk.filename.clone())
        .or_else(|| {
            response
                .documents
                .first()
                .map(|item| item.document.filename.clone())
        })
        .unwrap_or_else(|| document.filename.clone());
    document.chunks = response.chunks.clone();
    document.errors = response
        .documents
        .iter()
        .flat_map(|item| item.errors.iter())
        .filter_map(|error| error.error_message.clone())
        .collect();
    if let Some(item) = response.documents.first().cloned() {
        apply_export_document(document, item.document);
    }
    document.chunk_response = Some(response);
}

fn apply_json_result(document: &mut ConvertedDocument, value: Value) {
    let source = value.get("document").unwrap_or(&value);
    if let Some(object) = source.as_object() {
        document.markdown = object
            .get("md_content")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        document.text = object
            .get("text_content")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        document.html = object
            .get("html_content")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        document.doctags = object
            .get("doctags_content")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        document.doclang = object
            .get("doclang_content")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        document.json = object.get("json_content").cloned();
    }
    document.errors = collect_json_errors(&value);
}

fn collect_json_errors(value: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(message) = value.get("error_message").and_then(Value::as_str) {
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

fn select_output_content(
    document: &ConvertedDocument,
    output_format: OutputFormat,
) -> Result<Vec<u8>> {
    if output_format.is_archive() {
        return document.archive.clone().ok_or_else(|| {
            PdfConvertError::operation_error(
                "writing archive",
                "Docling did not return an application/zip response",
            )
        });
    }

    if output_format.is_chunk_output() {
        return document
            .chunk_response
            .as_ref()
            .map(serde_json::to_vec_pretty)
            .transpose()
            .map_err(PdfConvertError::from)?
            .ok_or_else(|| {
                PdfConvertError::operation_error(
                    "writing chunks",
                    "Docling did not return a chunk response",
                )
            });
    }

    match output_format {
        OutputFormat::Md => text_content(document.markdown.as_deref(), "markdown"),
        OutputFormat::Text => text_content(document.text.as_deref(), "text"),
        OutputFormat::Html => text_content(document.html.as_deref(), "html"),
        OutputFormat::Doctags => text_content(document.doctags.as_deref(), "doctags"),
        OutputFormat::Doclang => text_content(document.doclang.as_deref(), "doclang"),
        OutputFormat::Json => document
            .json
            .as_ref()
            .map(serde_json::to_vec_pretty)
            .transpose()
            .map_err(PdfConvertError::from)?
            .ok_or_else(|| {
                PdfConvertError::operation_error("writing json", "JSON output is empty")
            }),
        OutputFormat::Yaml
        | OutputFormat::HtmlSplitPage
        | OutputFormat::Vtt
        | OutputFormat::Dclx
        | OutputFormat::Chunks => unreachable!("handled above"),
    }
}

fn text_content(value: Option<&str>, name: &str) -> Result<Vec<u8>> {
    value.map(|value| value.as_bytes().to_vec()).ok_or_else(|| {
        PdfConvertError::operation_error(format!("writing {name}"), "output is empty")
    })
}
