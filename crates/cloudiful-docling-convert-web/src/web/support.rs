use std::str::FromStr;

use axum::http::StatusCode;
use cloudiful_docling_convert::{
    DoclingClient, DoclingRuntimeConfig, InputKind, OutputFormat, PdfConvertError,
    build_docling_client,
};

use crate::web::state::AppState;

pub fn sanitize_filename(name: &str) -> String {
    let name = name
        .split('/')
        .last()
        .unwrap_or(name)
        .split('\\')
        .last()
        .unwrap_or(name);

    let name = name
        .split('?')
        .next()
        .unwrap_or(name)
        .split('#')
        .next()
        .unwrap_or(name);

    let invalid_chars = ['<', '>', ':', '"', '|', '*', '\0'];
    let mut result: String = name
        .chars()
        .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
        .collect();

    result = result.trim().trim_matches('.').to_string();

    if result.is_empty() {
        return "downloaded".to_string();
    }

    result
}

pub fn parse_output_format(value: &str) -> Result<OutputFormat, PdfConvertError> {
    OutputFormat::from_str(value)
}

pub fn parse_input_format(value: &str) -> Result<InputKind, PdfConvertError> {
    InputKind::from_str(value)
}

pub fn resolve_input_kind(
    file_name: &str,
    media_type: &str,
    input_format: Option<&str>,
) -> Result<InputKind, StatusCode> {
    if let Some(input_format) = input_format {
        return parse_input_format(input_format).map_err(|_| StatusCode::BAD_REQUEST);
    }

    InputKind::from_filename_and_media_type(file_name, Some(media_type)).ok_or(
        if InputKind::requires_explicit_override(file_name, Some(media_type)) {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        },
    )
}

pub fn create_docling_client(state: &AppState) -> Result<DoclingClient, PdfConvertError> {
    build_docling_client(DoclingRuntimeConfig {
        docling_base_url: state.docling_base_url.clone(),
        openai_base_url: state.openai_base_url.clone(),
        vlm_pipeline_model: state.vlm_pipeline_model.clone(),
        picture_description_model: state.picture_description_model.clone(),
        code_formula_model: state.code_formula_model.clone(),
        api_key: std::env::var("OPENAI_API_KEY").ok(),
    })
}

pub fn output_content_type(format: &str) -> &'static str {
    match format {
        "json" => "application/json",
        "text" => "text/plain; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "doctags" => "text/plain; charset=utf-8",
        _ => "text/markdown; charset=utf-8",
    }
}
