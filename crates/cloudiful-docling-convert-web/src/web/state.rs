use log::warn;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversionTask {
    pub id: String,
    pub filename: String,
    pub status: TaskStatus,
    pub progress: u8,
    pub message: Option<String>,
    pub output_url: Option<String>,
    pub total_chunks: u32,
    pub completed_chunks: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub config: TaskConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskConfig {
    pub format: String,
    pub pages_per_file: u32,
    pub split_input: bool,
    pub split_by_bookmark: bool,
    pub chunking: bool,
    pub batch_size: usize,
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            format: "md".to_string(),
            pages_per_file: 5,
            split_input: true,
            split_by_bookmark: false,
            chunking: false,
            batch_size: 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub tasks: Arc<RwLock<HashMap<String, ConversionTask>>>,
    pub output_paths: Arc<RwLock<HashMap<String, PathBuf>>>,
    pub http_client: reqwest::Client,
    pub docling_base_url: String,
    pub openai_base_url: String,
    pub vlm_pipeline_model: String,
    pub picture_description_model: String,
    pub code_formula_model: String,
}

pub fn task_work_dir(task_id: &str) -> PathBuf {
    std::env::temp_dir()
        .join("cloudiful-docling-convert")
        .join(task_id)
}

async fn remove_file_if_exists(path: &Path) {
    match tokio::fs::remove_file(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => warn!("failed to remove file '{}': {}", path.display(), error),
    }
}

pub async fn cleanup_task_artifacts(task_id: &str, output_path: Option<PathBuf>) {
    if let Some(path) = output_path.as_deref() {
        remove_file_if_exists(path).await;
    }

    let work_dir = task_work_dir(task_id);
    match tokio::fs::remove_dir_all(&work_dir).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => warn!(
            "failed to remove task work dir '{}': {}",
            work_dir.display(),
            error
        ),
    }
}

impl AppState {
    pub fn new(
        docling_base_url: String,
        openai_base_url: String,
        vlm_pipeline_model: String,
        picture_description_model: String,
        code_formula_model: String,
    ) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            output_paths: Arc::new(RwLock::new(HashMap::new())),
            http_client: reqwest::Client::new(),
            docling_base_url,
            openai_base_url,
            vlm_pipeline_model,
            picture_description_model,
            code_formula_model,
        }
    }

    pub async fn create_task(
        &self,
        filename: String,
        config: TaskConfig,
        total_chunks: u32,
    ) -> String {
        let task_id = uuid::Uuid::new_v4().to_string();
        let task = ConversionTask {
            id: task_id.clone(),
            filename,
            status: TaskStatus::Pending,
            progress: 0,
            message: None,
            output_url: None,
            total_chunks,
            completed_chunks: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
            config,
        };

        let mut tasks = self.tasks.write().await;
        tasks.insert(task_id.clone(), task);
        task_id
    }

    pub async fn get_task(&self, task_id: &str) -> Option<ConversionTask> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).cloned()
    }

    pub async fn update_task(
        &self,
        task_id: &str,
        status: TaskStatus,
        progress: u8,
        message: Option<String>,
    ) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            let should_set_completed = matches!(status, TaskStatus::Completed | TaskStatus::Failed);
            task.status = status;
            task.progress = progress;
            task.message = message;
            if should_set_completed {
                task.completed_at = Some(chrono::Utc::now());
            }
            true
        } else {
            false
        }
    }

    pub async fn set_task_total_chunks(&self, task_id: &str, total_chunks: u32) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.total_chunks = total_chunks;
            if total_chunks > 0 && task.completed_chunks > total_chunks {
                task.completed_chunks = total_chunks;
            }
            true
        } else {
            false
        }
    }

    pub async fn update_task_chunk_progress(
        &self,
        task_id: &str,
        completed_chunks: u32,
        total_chunks: u32,
        progress: u8,
        message: Option<String>,
    ) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStatus::Processing;
            task.total_chunks = total_chunks;
            task.completed_chunks = completed_chunks.min(total_chunks);
            task.progress = progress;
            task.message = message;
            true
        } else {
            false
        }
    }

    pub async fn set_task_output(&self, task_id: &str, output_url: String) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStatus::Completed;
            task.progress = 100;
            task.completed_chunks = task.total_chunks.max(task.completed_chunks);
            task.message = Some("Conversion completed".to_string());
            task.output_url = Some(output_url);
            task.completed_at = Some(chrono::Utc::now());
            true
        } else {
            false
        }
    }

    pub async fn set_task_output_path(&self, task_id: &str, output_path: PathBuf) -> bool {
        let tasks = self.tasks.write().await;
        if !tasks.contains_key(task_id) {
            return false;
        }
        let mut output_paths = self.output_paths.write().await;
        output_paths.insert(task_id.to_string(), output_path);
        true
    }

    pub async fn get_task_output_path(&self, task_id: &str) -> Option<PathBuf> {
        let output_paths = self.output_paths.read().await;
        output_paths.get(task_id).cloned()
    }

    pub async fn list_tasks(&self) -> Vec<ConversionTask> {
        let tasks = self.tasks.read().await;
        let mut list: Vec<_> = tasks.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub async fn delete_task(&self, task_id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        let mut output_paths = self.output_paths.write().await;

        let output_path = output_paths.remove(task_id);
        let removed = tasks.remove(task_id).is_some();
        drop(output_paths);
        drop(tasks);

        if removed {
            cleanup_task_artifacts(task_id, output_path).await;
        }

        removed
    }
}
