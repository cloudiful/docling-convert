use std::time::{Duration, Instant};

use reqwest::multipart;
use serde_json::Value;

use crate::document::{ChunkerKind, ChunkingOptions, InputDocument, OutputFormat, PipelineKind};
use crate::error::{PdfConvertError, Result};
use crate::models::{ConversionStatus, TaskPostResponse, TaskStatusResponse};

use super::chunk::{build_convert_file_form, build_file_form};
use super::result::{DoclingResult, DoclingTaskResult, parse_response};
use super::source::{chunk_source_request, source_request};
use super::transport::{Transport, default_request_timeout, handle_response, retry_with_backoff};

#[derive(Debug, Clone)]
pub struct DoclingConfig {
    pub base_url: String,
    pub openai_base_url: String,
    pub vlm_pipeline_model: String,
    pub picture_description_model: String,
    pub code_formula_model: String,
    pub api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub tenant_id: Option<String>,
    pub request_timeout: Option<Duration>,
}

impl DoclingConfig {
    pub fn without_vlm(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            openai_base_url: String::new(),
            vlm_pipeline_model: String::new(),
            picture_description_model: String::new(),
            code_formula_model: String::new(),
            api_key: None,
            openai_api_key: None,
            tenant_id: None,
            request_timeout: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DoclingConvertRequest {
    pub output_formats: Vec<OutputFormat>,
    pub page_range: Option<(u32, u32)>,
    pub chunker: ChunkerKind,
    pub chunking: ChunkingOptions,
    pub pipeline: Option<PipelineKind>,
}

impl DoclingConvertRequest {
    pub fn for_outputs(output_formats: Vec<OutputFormat>) -> Self {
        Self {
            output_formats,
            page_range: None,
            chunker: ChunkerKind::None,
            chunking: ChunkingOptions::hybrid_defaults(),
            pipeline: None,
        }
    }

    pub fn with_chunker(mut self, chunker: ChunkerKind, options: ChunkingOptions) -> Self {
        self.chunker = chunker;
        self.chunking = options;
        self
    }

    pub fn with_pipeline(mut self, pipeline: Option<PipelineKind>) -> Self {
        self.pipeline = pipeline;
        self
    }
}

#[derive(Clone)]
pub struct DoclingClient {
    transport: Transport,
    result_body_limit: Option<usize>,
}

impl std::fmt::Debug for DoclingClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoclingClient")
            .field("base_url", &self.transport.config().base_url)
            .field("tenant_id", &self.transport.config().tenant_id)
            .finish_non_exhaustive()
    }
}

impl DoclingClient {
    pub fn new(config: DoclingConfig) -> Result<Self> {
        Ok(Self {
            transport: Transport::new(config)?,
            result_body_limit: None,
        })
    }

    pub fn new_with_result_body_limit(
        config: DoclingConfig,
        result_body_limit: usize,
    ) -> Result<Self> {
        if result_body_limit == 0 {
            return Err(PdfConvertError::validation_error(
                "result_body_limit",
                "value must be greater than 0",
            ));
        }
        Ok(Self {
            transport: Transport::new(config)?,
            result_body_limit: Some(result_body_limit),
        })
    }

    pub fn config(&self) -> &DoclingConfig {
        self.transport.config()
    }

    pub fn request_timeout(&self) -> Duration {
        self.config()
            .request_timeout
            .unwrap_or_else(default_request_timeout)
    }

    pub async fn convert_file(
        &self,
        input: &InputDocument,
        request: &DoclingConvertRequest,
    ) -> Result<DoclingResult> {
        let operation = || async {
            let form = self.build_form(input, request)?;
            let path = match request.chunker {
                ChunkerKind::None => "convert/file",
                ChunkerKind::Hybrid => "chunk/hybrid/file",
                ChunkerKind::Hierarchical => "chunk/hierarchical/file",
            };
            let response = self
                .transport
                .request(reqwest::Method::POST, path)
                .multipart(form)
                .send()
                .await
                .map_err(PdfConvertError::from)?;
            parse_response(response, "Docling file conversion", self.result_body_limit).await
        };

        retry_with_backoff(operation, "docling_convert_file").await
    }

    pub async fn submit_file_async(
        &self,
        input: &InputDocument,
        request: &DoclingConvertRequest,
    ) -> Result<String> {
        let operation = || async {
            let form = self.build_form(input, request)?;
            let path = match request.chunker {
                ChunkerKind::None => "convert/file/async",
                ChunkerKind::Hybrid => "chunk/hybrid/file/async",
                ChunkerKind::Hierarchical => "chunk/hierarchical/file/async",
            };
            let response = self
                .transport
                .request(reqwest::Method::POST, path)
                .multipart(form)
                .send()
                .await
                .map_err(PdfConvertError::from)?;
            let response = handle_response(response, "Docling async submission").await?;
            let task = response.json::<TaskPostResponse>().await.map_err(|error| {
                PdfConvertError::parse_error("Docling async submission response", error.to_string())
            })?;
            Ok(task.task_id)
        };

        retry_with_backoff(operation, "docling_submit_file_async").await
    }

    pub async fn convert_source(
        &self,
        url: &str,
        input_kind: crate::document::InputKind,
        request: &DoclingConvertRequest,
    ) -> Result<DoclingResult> {
        let operation = || async {
            let path = match request.chunker {
                ChunkerKind::None => "convert/source",
                ChunkerKind::Hybrid => "chunk/hybrid/source",
                ChunkerKind::Hierarchical => "chunk/hierarchical/source",
            };
            let body = if request.chunker == ChunkerKind::None {
                serde_json::to_value(source_request(url, input_kind, request))?
            } else {
                serde_json::to_value(chunk_source_request(url, input_kind, request)?)?
            };
            let response = self
                .transport
                .request(reqwest::Method::POST, path)
                .json(&body)
                .send()
                .await
                .map_err(PdfConvertError::from)?;
            parse_response(
                response,
                "Docling source conversion",
                self.result_body_limit,
            )
            .await
        };

        retry_with_backoff(operation, "docling_convert_source").await
    }

    pub async fn submit_source_async(
        &self,
        url: &str,
        input_kind: crate::document::InputKind,
        request: &DoclingConvertRequest,
    ) -> Result<String> {
        let operation = || async {
            let path = match request.chunker {
                ChunkerKind::None => "convert/source/async",
                ChunkerKind::Hybrid => "chunk/hybrid/source/async",
                ChunkerKind::Hierarchical => "chunk/hierarchical/source/async",
            };
            let body = if request.chunker == ChunkerKind::None {
                serde_json::to_value(source_request(url, input_kind, request))?
            } else {
                serde_json::to_value(chunk_source_request(url, input_kind, request)?)?
            };
            let response = self
                .transport
                .request(reqwest::Method::POST, path)
                .json(&body)
                .send()
                .await
                .map_err(PdfConvertError::from)?;
            let response = handle_response(response, "Docling source async submission").await?;
            let task = response.json::<TaskPostResponse>().await.map_err(|error| {
                PdfConvertError::parse_error(
                    "Docling source async submission response",
                    error.to_string(),
                )
            })?;
            Ok(task.task_id)
        };

        retry_with_backoff(operation, "docling_submit_source_async").await
    }

    pub async fn wait_for_result(&self, task_id: &str) -> Result<DoclingTaskResult> {
        self.wait_for_result_with_progress(task_id, |_| async {})
            .await
    }

    pub async fn wait_for_result_with_progress<F, Fut>(
        &self,
        task_id: &str,
        mut on_status: F,
    ) -> Result<DoclingTaskResult>
    where
        F: FnMut(TaskStatusResponse) -> Fut + Send,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let deadline = Instant::now() + self.request_timeout();
        loop {
            let status = self.poll_task_status(task_id).await?;
            on_status(status.clone()).await;
            if status.task_status.is_terminal() {
                if matches!(
                    status.task_status,
                    ConversionStatus::Failure | ConversionStatus::Skipped
                ) {
                    return Err(task_failure_error(&status));
                }
                let result = self.get_task_result(task_id).await?;
                return Ok(DoclingTaskResult {
                    status: status.task_status,
                    result,
                    errors: task_status_errors(&status),
                });
            }

            if Instant::now() >= deadline {
                return Err(PdfConvertError::operation_error(
                    "waiting for Docling task",
                    format!(
                        "task {task_id} did not reach a terminal state within {:?}",
                        self.request_timeout()
                    ),
                ));
            }
        }
    }

    pub async fn poll_task_status(&self, task_id: &str) -> Result<TaskStatusResponse> {
        let operation = || async {
            let path = format!("status/poll/{task_id}");
            let response = self
                .transport
                .request(reqwest::Method::GET, &format!("{path}?wait=30"))
                .send()
                .await
                .map_err(PdfConvertError::from)?;
            let response = handle_response(response, "Polling task status").await?;
            let text = response.text().await.map_err(PdfConvertError::from)?;
            serde_json::from_str::<TaskStatusResponse>(&text).map_err(|error| {
                PdfConvertError::parse_error(
                    "task status response",
                    format!("task {task_id} returned invalid response: {error}; body: {text}"),
                )
            })
        };

        retry_with_backoff(operation, &format!("check_task_status({task_id})")).await
    }

    pub async fn check_task_status(&self, task_id: &str) -> Result<bool> {
        let status = self.poll_task_status(task_id).await?;
        if matches!(
            status.task_status,
            ConversionStatus::Failure | ConversionStatus::Skipped
        ) {
            return Err(PdfConvertError::api_task_failed(
                format!("{:?}", status.task_status),
                task_status_error(&status),
            ));
        }
        Ok(status.task_status.is_successful())
    }

    pub async fn get_task_result(&self, task_id: &str) -> Result<DoclingResult> {
        self.get_task_result_with_connection(task_id, false).await
    }

    pub async fn get_task_result_with_connection(
        &self,
        task_id: &str,
        close_connection: bool,
    ) -> Result<DoclingResult> {
        let operation = || async {
            let path = format!("result/{task_id}");
            let mut request = self.transport.request(reqwest::Method::GET, &path);
            if close_connection {
                request = request.header(reqwest::header::CONNECTION, "close");
            }
            let response = request.send().await.map_err(PdfConvertError::from)?;
            parse_response(response, "Fetching task result", self.result_body_limit).await
        };

        retry_with_backoff(operation, &format!("get_task_result({task_id})")).await
    }

    pub async fn get_task_result_value(&self, task_id: &str) -> Result<Value> {
        self.get_task_result(task_id)
            .await?
            .into_json()
            .ok_or_else(|| {
                PdfConvertError::operation_error(
                    "reading task result",
                    "task result is a ZIP response and has no JSON value",
                )
            })
    }

    pub(crate) fn build_form(
        &self,
        input: &InputDocument,
        request: &DoclingConvertRequest,
    ) -> Result<multipart::Form> {
        match request.chunker {
            ChunkerKind::None => build_convert_file_form(self, input, request),
            ChunkerKind::Hybrid | ChunkerKind::Hierarchical => build_file_form(input, request),
        }
    }
}

fn task_status_error(status: &TaskStatusResponse) -> String {
    status
        .error_message
        .clone()
        .or_else(|| {
            status
                .failure
                .as_ref()
                .map(|failure| failure.message.clone())
        })
        .unwrap_or_else(|| format!("Docling task status is {:?}", status.task_status))
}

fn task_status_errors(status: &TaskStatusResponse) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(message) = status.error_message.as_deref() {
        errors.push(message.to_string());
    }
    if let Some(failure) = status.failure.as_ref() {
        errors.push(failure.message.clone());
    }
    errors.sort();
    errors.dedup();
    errors
}

fn task_failure_error(status: &TaskStatusResponse) -> PdfConvertError {
    PdfConvertError::api_task_failed(
        format!("{:?}", status.task_status),
        task_status_error(status),
    )
}
