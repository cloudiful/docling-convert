use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionStatus {
    Pending,
    Started,
    Failure,
    Success,
    PartialSuccess,
    Skipped,
}

impl ConversionStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Pending | Self::Started)
    }

    pub fn is_successful(self) -> bool {
        matches!(self, Self::Success | Self::PartialSuccess)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TaskPostResponse {
    pub task_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TaskStatusResponse {
    pub task_id: String,
    pub task_type: Option<String>,
    pub task_status: ConversionStatus,
    #[serde(default)]
    pub task_position: Option<u64>,
    #[serde(default)]
    pub task_meta: Option<TaskProcessingMeta>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub failure: Option<PublicFailureInfo>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct TaskProcessingMeta {
    pub num_docs: u64,
    #[serde(default)]
    pub num_processed: u64,
    #[serde(default)]
    pub num_succeeded: u64,
    #[serde(default)]
    pub num_partially_succeeded: u64,
    #[serde(default)]
    pub num_failed: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PublicFailureInfo {
    pub category: String,
    pub message: String,
    pub retryable: bool,
    pub phase: String,
    #[serde(default)]
    pub details: Option<Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TaskFailureResult {
    #[serde(default = "task_failure_kind")]
    pub kind: String,
    pub failure: PublicFailureInfo,
}

fn task_failure_kind() -> String {
    "TaskFailureResult".to_string()
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DoclingErrorItem {
    #[serde(default)]
    pub component_type: Option<String>,
    #[serde(default)]
    pub module_name: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub page_no: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ExportDocumentResponse {
    pub filename: String,
    #[serde(default)]
    pub md_content: Option<String>,
    #[serde(default)]
    pub json_content: Option<Value>,
    #[serde(default)]
    pub html_content: Option<String>,
    #[serde(default)]
    pub text_content: Option<String>,
    #[serde(default)]
    pub doctags_content: Option<String>,
    #[serde(default)]
    pub doclang_content: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocumentResultItem {
    #[serde(rename = "content", alias = "document")]
    pub document: ExportDocumentResponse,
    pub status: ConversionStatus,
    #[serde(default)]
    pub errors: Vec<DoclingErrorItem>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ConvertDocumentResponse {
    pub document: ExportDocumentResponse,
    pub status: ConversionStatus,
    #[serde(default)]
    pub errors: Vec<DoclingErrorItem>,
    #[serde(default)]
    pub processing_time: f64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DoclingChunk {
    pub filename: String,
    pub chunk_index: u64,
    pub text: String,
    #[serde(default)]
    pub raw_text: Option<String>,
    #[serde(default)]
    pub num_tokens: Option<u64>,
    #[serde(default)]
    pub headings: Option<Vec<String>>,
    #[serde(default)]
    pub captions: Option<Vec<String>>,
    #[serde(default)]
    pub doc_items: Vec<String>,
    #[serde(default)]
    pub page_numbers: Option<Vec<u64>>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ChunkDocumentResponse {
    #[serde(default)]
    pub chunks: Vec<DoclingChunk>,
    #[serde(default)]
    pub documents: Vec<DocumentResultItem>,
    #[serde(default)]
    pub processing_time: f64,
}
