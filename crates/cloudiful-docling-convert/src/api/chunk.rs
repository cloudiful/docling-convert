use bytes::Bytes;
use reqwest::multipart;

use crate::document::{ChunkerKind, InputDocument};
use crate::error::{PdfConvertError, Result};

use super::docling::{DoclingClient, DoclingConvertRequest};

pub(crate) fn build_convert_file_form(
    client: &DoclingClient,
    input: &InputDocument,
    request: &DoclingConvertRequest,
) -> Result<multipart::Form> {
    let input_kind = input.kind()?;
    let part = multipart::Part::stream(reqwest::Body::from(Bytes::clone(&input.bytes)))
        .file_name(input.filename.clone())
        .mime_str(&input.media_type)
        .map_err(|error| {
            PdfConvertError::api_error(None, format!("failed to create multipart file: {error}"))
        })?;

    let mut form = multipart::Form::new()
        .part("files", part)
        .text("target_type", target_type(request).to_string())
        .text("from_formats", input_kind.from_formats_value().to_string());
    for format in &request.output_formats {
        form = form.text("to_formats", format.as_api_value().to_string());
    }
    if let Some((start_page, end_page)) = request.page_range {
        form = form.text("page_range", start_page.to_string());
        form = form.text("page_range", end_page.to_string());
    }
    if let Some(pipeline) = request.pipeline {
        form = form.text("pipeline", pipeline.to_string());
    }

    if input_kind.supports_vlm()
        && let Some(vlm_config) = client.config().resolved_vlm_config()?
    {
        form = client.apply_vlm_config(form, &vlm_config)?;
    }

    Ok(form)
}

pub(crate) fn build_file_form(
    input: &InputDocument,
    request: &DoclingConvertRequest,
) -> Result<multipart::Form> {
    let input_kind = input.kind()?;
    let part = multipart::Part::stream(reqwest::Body::from(Bytes::clone(&input.bytes)))
        .file_name(input.filename.clone())
        .mime_str(&input.media_type)
        .map_err(|error| {
            PdfConvertError::api_error(None, format!("failed to create multipart file: {error}"))
        })?;

    let mut form = multipart::Form::new()
        .part("files", part)
        .text("include_converted_doc", "false")
        .text("target_type", target_type(request).to_string())
        .text(
            "convert_from_formats",
            input_kind.from_formats_value().to_string(),
        );

    for format in &request.output_formats {
        form = form.text("convert_to_formats", format.as_api_value().to_string());
    }

    if let Some((start_page, end_page)) = request.page_range {
        form = form.text("convert_page_range", start_page.to_string());
        form = form.text("convert_page_range", end_page.to_string());
    }
    if let Some(pipeline) = request.pipeline {
        form = form.text("convert_pipeline", pipeline.to_string());
    }

    let options = &request.chunking;
    form = form
        .text(
            "chunking_use_markdown_tables",
            options.use_markdown_tables.to_string(),
        )
        .text(
            "chunking_use_markdown_images",
            options.use_markdown_images.to_string(),
        )
        .text(
            "chunking_image_placeholder",
            options.image_placeholder.clone(),
        )
        .text(
            "chunking_include_raw_text",
            options.include_raw_text.to_string(),
        );

    if let Some(max_tokens) = options.max_tokens {
        form = form.text("chunking_max_tokens", max_tokens.to_string());
    }
    if let Some(tokenizer) = options.tokenizer.as_deref() {
        form = form.text("chunking_tokenizer", tokenizer.to_string());
    }
    if matches!(request.chunker, ChunkerKind::Hybrid) {
        form = form.text("chunking_merge_peers", options.merge_peers.to_string());
    }

    Ok(form)
}

pub(crate) fn target_type(request: &DoclingConvertRequest) -> &'static str {
    if request
        .output_formats
        .iter()
        .any(|format| format.is_archive())
    {
        "zip"
    } else {
        "inbody"
    }
}
