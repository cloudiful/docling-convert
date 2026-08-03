use std::path::{Path, PathBuf};

use cloudiful_docling_convert::{ConversionBehavior, PdfConvertError, TaskStatusResponse};

use super::super::state::{
    AppState, TaskConfig, TaskStatus, cleanup_task_artifacts, task_work_dir,
};

pub fn behavior_from_task_config(config: &TaskConfig) -> ConversionBehavior {
    ConversionBehavior {
        chunker: config.chunker,
        chunking: config.chunking_options.clone(),
        pipeline: config.pipeline,
    }
}

pub async fn path_exists(path: &Path) -> Result<bool, PdfConvertError> {
    match tokio::fs::metadata(path).await {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(PdfConvertError::io_error(
            format!("reading file metadata: {}", path.display()),
            error,
        )),
    }
}

pub async fn ensure_task_temp_dir(task_id: &str) -> Result<PathBuf, PdfConvertError> {
    let temp_dir = task_work_dir(task_id);
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|error| {
            PdfConvertError::io_error(
                format!("creating task temp directory {}", temp_dir.display()),
                error,
            )
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

pub async fn update_docling_status(state: &AppState, task_id: &str, status: &TaskStatusResponse) {
    let Some(meta) = status.task_meta.as_ref() else {
        if let Some(message) = status.error_message.as_deref() {
            update_processing_status(state, task_id, 20, message).await;
        }
        return;
    };

    let total = meta.num_docs.min(u32::MAX as u64) as u32;
    let completed = meta.num_processed.min(total as u64) as u32;
    let progress = if total == 0 {
        10
    } else {
        (10 + completed.saturating_mul(80) / total).min(90) as u8
    };
    let message = status
        .error_message
        .clone()
        .or_else(|| {
            status
                .failure
                .as_ref()
                .map(|failure| failure.message.clone())
        })
        .unwrap_or_else(|| format!("Docling processed {completed}/{total} documents"));
    state
        .update_task_chunk_progress(task_id, completed, total, progress, Some(message))
        .await;
}

pub async fn finalize_task_output(
    state: &AppState,
    task_id: &str,
    output_file: PathBuf,
    status: TaskStatus,
    total_chunks: u32,
    message: Option<String>,
) -> Result<(), PdfConvertError> {
    if path_exists(&output_file).await? {
        let output_url = format!("/api/download/{task_id}");
        if !state
            .set_task_output_path(task_id, output_file.clone())
            .await
        {
            cleanup_task_artifacts(task_id, Some(output_file)).await;
            return Ok(());
        }
        if !state
            .set_task_output_status(task_id, output_url, status, total_chunks, message)
            .await
        {
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
