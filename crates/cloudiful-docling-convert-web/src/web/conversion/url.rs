use std::path::Path;

use cloudiful_docling_convert::{
    DocumentConverter, InputKind, PdfConvertError, RemoteConvertOptions,
};

use super::super::state::{AppState, TaskConfig, TaskStatus};
use super::super::support::{create_docling_client, parse_input_format, parse_output_format};
use super::support::{
    behavior_from_task_config, ensure_task_temp_dir, finalize_task_output, update_docling_status,
    update_processing_status,
};

pub async fn process_url_conversion(
    state: AppState,
    task_id: String,
    url: String,
    fallback_file_name: String,
    config: TaskConfig,
) -> Result<(), PdfConvertError> {
    let input_kind = config
        .input_format
        .as_deref()
        .map(parse_input_format)
        .transpose()?
        .or_else(|| InputKind::from_path(Path::new(&fallback_file_name)))
        .ok_or_else(|| {
            PdfConvertError::validation_error(
                "input_format",
                "URL filename has no supported extension; provide input_format",
            )
        })?;
    let output_format = parse_output_format(&config.format)?;
    let behavior = behavior_from_task_config(&config);
    let temp_dir = ensure_task_temp_dir(&task_id).await?;
    update_processing_status(
        &state,
        &task_id,
        10,
        format!("Submitting {input_kind:?} source to Docling..."),
    )
    .await;

    let converter = DocumentConverter::new(create_docling_client(&state)?);
    let result = converter
        .convert_source_to_file_async_with_docling_progress(
            &url,
            fallback_file_name,
            input_kind,
            vec![output_format],
            &RemoteConvertOptions {
                chunker: behavior.chunker,
                chunking: behavior.chunking,
                pipeline: behavior.pipeline,
            },
            temp_dir,
            output_format,
            true,
            |status| {
                let state = state.clone();
                let task_id = task_id.clone();
                async move { update_docling_status(&state, &task_id, &status).await }
            },
        )
        .await?;

    let total_chunks = result.document.chunks.len().max(1).min(u32::MAX as usize) as u32;
    let status = if result.document.errors.is_empty() {
        TaskStatus::Completed
    } else {
        TaskStatus::Partial
    };
    let message = if result.document.errors.is_empty() {
        Some("Conversion completed".to_string())
    } else {
        Some(result.document.errors.join("; "))
    };
    finalize_task_output(
        &state,
        &task_id,
        result.output_paths.into_iter().next().ok_or_else(|| {
            PdfConvertError::operation_error("output", "no output file was produced")
        })?,
        status,
        total_chunks,
        message,
    )
    .await
}
