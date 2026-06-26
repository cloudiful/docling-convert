use cloudiful_docling_convert::{
    ConvertRequest, DocumentConverter, FileConvertRequest, InputDocument, InputKind,
    PdfConvertError, PdfInfo, build_convert_options,
};

use super::super::state::{AppState, TaskConfig};
use super::super::support::{create_docling_client, parse_output_format};
use super::support::{
    behavior_from_task_config, ensure_task_temp_dir, finalize_task_output, set_task_total_chunks,
    total_chunks_for_input, update_chunk_progress, update_processing_status,
};

pub async fn process_file_conversion(
    state: AppState,
    task_id: String,
    input: InputDocument,
    config: TaskConfig,
) -> Result<(), PdfConvertError> {
    let input_kind = input.kind()?;
    let total_chunks =
        total_chunks_for_input(&input, &config).unwrap_or_else(|_| match input_kind {
            InputKind::Pdf => 0,
            _ => 1,
        });
    if !set_task_total_chunks(&state, &task_id, total_chunks, input_kind.reading_label()).await {
        return Ok(());
    }

    if matches!(input_kind, InputKind::Pdf) {
        let pdf_info = PdfInfo::load_from_bytes(input.bytes.as_ref())?;
        update_processing_status(
            &state,
            &task_id,
            20,
            format!("PDF loaded: {} pages", pdf_info.total_pages),
        )
        .await;
    } else {
        update_processing_status(
            &state,
            &task_id,
            20,
            format!("Detected {:?} input", input_kind),
        )
        .await;
    }

    let temp_dir = ensure_task_temp_dir(&task_id).await?;
    update_processing_status(&state, &task_id, 30, "Starting conversion...").await;

    let output_format = parse_output_format(&config.format)?;
    let converter = DocumentConverter::new(create_docling_client(&state)?);
    let progress_state = state.clone();
    let progress_task_id = task_id.clone();
    let result = converter
        .convert_to_file_with_progress(
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
            move |completed_chunks, total_chunks| {
                let progress_state = progress_state.clone();
                let progress_task_id = progress_task_id.clone();
                async move {
                    update_chunk_progress(
                        &progress_state,
                        &progress_task_id,
                        completed_chunks,
                        total_chunks,
                        format!("Processed chunk {}/{}", completed_chunks, total_chunks),
                    )
                    .await;
                }
            },
        )
        .await?;

    update_processing_status(&state, &task_id, 90, "Finalizing output...").await;

    let output_file =
        result.output_paths.into_iter().next().ok_or_else(|| {
            PdfConvertError::operation_error("output", "no output file was produced")
        })?;

    finalize_task_output(&state, &task_id, output_file).await
}
