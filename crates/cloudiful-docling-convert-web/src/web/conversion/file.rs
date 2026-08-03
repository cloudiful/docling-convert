use cloudiful_docling_convert::{
    ConvertRequest, DocumentConverter, FileConvertRequest, InputDocument, PdfConvertError,
    build_convert_options,
};

use super::super::state::{AppState, TaskConfig, TaskStatus};
use super::super::support::{create_docling_client, parse_output_format};
use super::support::{
    behavior_from_task_config, ensure_task_temp_dir, finalize_task_output, update_docling_status,
    update_processing_status,
};

pub async fn process_file_conversion(
    state: AppState,
    task_id: String,
    input: InputDocument,
    config: TaskConfig,
) -> Result<(), PdfConvertError> {
    let input_kind = input.kind()?;
    update_processing_status(
        &state,
        &task_id,
        10,
        format!("Submitting {:?} to Docling...", input_kind),
    )
    .await;

    let temp_dir = ensure_task_temp_dir(&task_id).await?;
    let output_format = parse_output_format(&config.format)?;
    let converter = DocumentConverter::new(create_docling_client(&state)?);
    let result = converter
        .convert_to_file_async_with_docling_progress(
            FileConvertRequest {
                request: ConvertRequest {
                    input,
                    output_formats: vec![output_format],
                    options: build_convert_options(
                        input_kind,
                        &behavior_from_task_config(&config),
                    )?,
                },
                output_dir: temp_dir,
                selected_output: output_format,
                overwrite: true,
            },
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
