use std::path::{Path, PathBuf};

use cloudiful_docling_convert::{
    ConversionBehavior, InputDocument, InputKind, PdfConvertError, count_input_chunks,
};

use super::super::state::{
    AppState, TaskConfig, TaskStatus, cleanup_task_artifacts, task_work_dir,
};

pub const MAX_REMOTE_DOWNLOAD_BYTES: usize = 100 * 1024 * 1024;

pub fn behavior_from_task_config(config: &TaskConfig) -> ConversionBehavior {
    ConversionBehavior {
        pages_per_file: config.pages_per_file,
        split_input: config.split_input,
        split_by_bookmark: config.split_by_bookmark,
        chunking: config.chunking,
        batch_size: config.batch_size,
    }
}

pub async fn path_exists(path: &Path) -> Result<bool, PdfConvertError> {
    match tokio::fs::metadata(path).await {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(PdfConvertError::io_error(
            format!("reading file metadata: {}", path.display()),
            e,
        )),
    }
}

pub async fn ensure_task_temp_dir(task_id: &str) -> Result<PathBuf, PdfConvertError> {
    let temp_dir = task_work_dir(task_id);
    if let Some(parent) = temp_dir.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            PdfConvertError::io_error(format!("creating parent directory {:?}: {}", parent, e), e)
        })?;
    }
    tokio::fs::create_dir_all(&temp_dir).await.map_err(|e| {
        let base_temp = std::env::temp_dir();
        let msg = if !base_temp.exists() {
            format!(
                "Temp directory '{}' does not exist. Please check TMPDIR environment variable or create {} manually",
                base_temp.display(),
                base_temp.display()
            )
        } else {
            format!("creating temp directory {:?}: {}", temp_dir, e)
        };
        PdfConvertError::io_error(msg, e)
    })?;

    Ok(temp_dir)
}

pub async fn update_processing_status(
    state: &AppState,
    task_id: &str,
    progress: u8,
    message: impl Into<String>,
) {
    state
        .update_task(
            task_id,
            TaskStatus::Processing,
            progress,
            Some(message.into()),
        )
        .await;
}

pub async fn update_chunk_progress(
    state: &AppState,
    task_id: &str,
    completed_chunks: usize,
    total_chunks: usize,
    message: impl Into<String>,
) {
    let total_chunks = total_chunks.min(u32::MAX as usize) as u32;
    let completed_chunks = completed_chunks.min(total_chunks as usize) as u32;
    let progress = progress_from_chunks(completed_chunks, total_chunks);
    state
        .update_task_chunk_progress(
            task_id,
            completed_chunks,
            total_chunks,
            progress,
            Some(message.into()),
        )
        .await;
}

pub async fn set_task_total_chunks(
    state: &AppState,
    task_id: &str,
    total_chunks: u32,
    message: impl Into<String>,
) -> bool {
    if !state.set_task_total_chunks(task_id, total_chunks).await {
        return false;
    }
    update_processing_status(state, task_id, 10, message).await;
    true
}

pub async fn finalize_task_output(
    state: &AppState,
    task_id: &str,
    output_file: PathBuf,
) -> Result<(), PdfConvertError> {
    if path_exists(&output_file).await? {
        let output_url = format!("/api/download/{}", task_id);
        if !state
            .set_task_output_path(task_id, output_file.clone())
            .await
        {
            cleanup_task_artifacts(task_id, Some(output_file)).await;
            return Ok(());
        }
        if !state.set_task_output(task_id, output_url).await {
            cleanup_task_artifacts(task_id, Some(output_file)).await;
        }
        return Ok(());
    }

    Err(PdfConvertError::validation_error(
        "output",
        "Output file not created",
    ))
}

pub fn spawn_conversion_task<F>(state: AppState, task_id: String, task: F)
where
    F: std::future::Future<Output = Result<(), PdfConvertError>> + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(error) = task.await {
            let _ = state
                .update_task(&task_id, TaskStatus::Failed, 0, Some(error.to_string()))
                .await;
        }
    });
}

pub fn validate_pdf_download(
    input_kind: InputKind,
    file_data: &[u8],
) -> Result<(), PdfConvertError> {
    if !matches!(input_kind, InputKind::Pdf) {
        return Ok(());
    }

    if file_data.len() < 5 {
        return Err(PdfConvertError::validation_error(
            "url",
            "Downloaded file is too small to be a valid PDF",
        ));
    }

    let header = std::str::from_utf8(&file_data[..4]).unwrap_or("");
    if header != "%PDF" {
        let actual_header = &file_data[..std::cmp::min(4, file_data.len())];
        let actual_str = String::from_utf8_lossy(actual_header);
        return Err(PdfConvertError::validation_error(
            "url",
            format!(
                "Invalid PDF file: file header is '{}' (expected '%PDF'). The URL may point to an HTML page or redirect instead of a PDF file. Please verify the URL points directly to a PDF document.",
                actual_str
            ),
        ));
    }

    Ok(())
}

pub fn total_chunks_for_input(
    input: &InputDocument,
    config: &TaskConfig,
) -> Result<u32, PdfConvertError> {
    let total = count_input_chunks(input, &behavior_from_task_config(config))?.max(1);
    Ok(total.min(u32::MAX as usize) as u32)
}

fn progress_from_chunks(completed_chunks: u32, total_chunks: u32) -> u8 {
    if total_chunks == 0 {
        return 10;
    }

    let ratio = completed_chunks as f32 / total_chunks as f32;
    let progress = 30.0 + ratio * 60.0;
    progress.round().clamp(30.0, 90.0) as u8
}
