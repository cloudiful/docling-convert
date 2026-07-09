use std::path::{Path, PathBuf};

use serde_json::Value;
use tokio::fs;

use super::DocumentConverter;
use crate::document::{
    ConvertedChunk, ConvertedDocument, ConvertedDocumentMetadata, InputDocument, InputKind,
    OutputFormat,
};
use crate::error::{PdfConvertError, Result};
use crate::models::{Bookmark, ChunkMetadata};

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
            fs::create_dir_all(parent).await.map_err(|e| {
                PdfConvertError::io_error(
                    format!("creating output directory: {}", parent.display()),
                    e,
                )
            })?;
        }

        let content = Self::select_output_content(document, output_format)?;
        fs::write(output_path, content).await.map_err(|e| {
            PdfConvertError::io_error(format!("writing output file: {}", output_path.display()), e)
        })?;

        Ok(())
    }

    fn select_output_content(
        document: &ConvertedDocument,
        output_format: OutputFormat,
    ) -> Result<Vec<u8>> {
        match output_format {
            OutputFormat::Md => document
                .markdown
                .as_ref()
                .map(|value| value.as_bytes().to_vec())
                .ok_or_else(|| {
                    PdfConvertError::operation_error("writing markdown", "markdown output is empty")
                }),
            OutputFormat::Text => document
                .text
                .as_ref()
                .map(|value| value.as_bytes().to_vec())
                .ok_or_else(|| {
                    PdfConvertError::operation_error("writing text", "text output is empty")
                }),
            OutputFormat::Html => document
                .html
                .as_ref()
                .map(|value| value.as_bytes().to_vec())
                .ok_or_else(|| {
                    PdfConvertError::operation_error("writing html", "html output is empty")
                }),
            OutputFormat::Doctags => document
                .doctags
                .as_ref()
                .map(|value| value.as_bytes().to_vec())
                .ok_or_else(|| {
                    PdfConvertError::operation_error("writing doctags", "doctags output is empty")
                }),
            OutputFormat::Json => document
                .json
                .as_ref()
                .map(serde_json::to_vec_pretty)
                .transpose()
                .map_err(PdfConvertError::from)?
                .ok_or_else(|| {
                    PdfConvertError::operation_error("writing json", "json output is empty")
                }),
        }
    }

    pub(super) fn chunk_from_result(
        metadata: Option<ChunkMetadata>,
        raw_result: Value,
    ) -> ConvertedChunk {
        ConvertedChunk {
            metadata,
            markdown: extract_document_field(&raw_result, "md_content"),
            text: extract_document_field(&raw_result, "text_content"),
            json: raw_result
                .get("document")
                .and_then(|document| document.get("json_content"))
                .cloned(),
            html: extract_document_field(&raw_result, "html_content"),
            doctags: extract_document_field(&raw_result, "doctags_content"),
            raw_result,
        }
    }

    pub(super) fn assemble_document(
        input: &InputDocument,
        input_kind: InputKind,
        page_count: Option<u32>,
        outlines: Vec<Bookmark>,
        chunks: Vec<ConvertedChunk>,
    ) -> ConvertedDocument {
        let markdown = join_optional_chunks(chunks.iter().map(|chunk| chunk.markdown.as_deref()));
        let text = join_optional_chunks(chunks.iter().map(|chunk| chunk.text.as_deref()));
        let html = join_optional_chunks(chunks.iter().map(|chunk| chunk.html.as_deref()));
        let doctags = join_optional_chunks(chunks.iter().map(|chunk| chunk.doctags.as_deref()));
        let json_values: Vec<Value> = chunks
            .iter()
            .filter_map(|chunk| chunk.json.clone())
            .collect();
        let json = match json_values.len() {
            0 => None,
            1 => json_values.into_iter().next(),
            _ => Some(Value::Array(json_values)),
        };

        ConvertedDocument {
            filename: input.filename.clone(),
            markdown,
            text,
            json,
            html,
            doctags,
            chunks,
            metadata: ConvertedDocumentMetadata {
                input_kind,
                media_type: input.media_type.clone(),
                page_count,
                outlines,
            },
            errors: Vec::new(),
        }
    }
}

fn extract_document_field(raw_result: &Value, field: &str) -> Option<String> {
    raw_result
        .get("document")
        .and_then(|document| document.get(field))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn join_optional_chunks<'a>(values: impl Iterator<Item = Option<&'a str>>) -> Option<String> {
    let mut collected = Vec::new();
    for value in values.flatten() {
        collected.push(value.trim_end_matches('\n').to_string());
    }

    if collected.is_empty() {
        None
    } else {
        Some(collected.join("\n"))
    }
}
