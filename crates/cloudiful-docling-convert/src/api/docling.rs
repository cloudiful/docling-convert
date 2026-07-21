use std::time::Duration;

use bytes::Bytes;
use reqwest::multipart;
use serde_json::Value;

use crate::document::{InputDocument, OutputFormat};
use crate::error::{PdfConvertError, Result};
use crate::models::vlm::{OpenRouterConfigBuilder, VlmConvertOptions};
use crate::models::{
    CodeFormulaVlmOptions, PictureDescriptionVlmEngineOptions, TaskPostResponse, TaskStatusResponse,
};

use super::client::{
    extract_error_details, get_request, get_request_with_conn_close, handle_response,
    retry_with_backoff,
};
use super::vlm_config::ResolvedVlmConfig;

#[derive(Debug, Clone)]
pub struct DoclingConfig {
    pub base_url: String,
    pub openai_base_url: String,
    pub vlm_pipeline_model: String,
    pub picture_description_model: String,
    pub code_formula_model: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DoclingConvertRequest {
    pub output_formats: Vec<OutputFormat>,
    pub page_range: Option<(u32, u32)>,
    pub chunking: bool,
}

impl DoclingConvertRequest {
    pub fn for_outputs(output_formats: Vec<OutputFormat>) -> Self {
        Self {
            output_formats,
            page_range: None,
            chunking: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DoclingClient {
    http_client: reqwest::Client,
    config: DoclingConfig,
}

impl DoclingClient {
    pub fn new(config: DoclingConfig) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .tcp_keepalive(Duration::from_secs(60))
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| PdfConvertError::api_error(None, e.to_string()))?;

        Ok(Self {
            http_client,
            config,
        })
    }

    pub fn config(&self) -> &DoclingConfig {
        &self.config
    }

    pub async fn convert_file(
        &self,
        input: &InputDocument,
        request: &DoclingConvertRequest,
    ) -> Result<Value> {
        let operation = || async {
            let form = self.build_form(input, request)?;
            let url = format!("{}/convert/file", self.config.base_url);
            let response = self
                .http_client
                .post(&url)
                .multipart(form)
                .send()
                .await
                .map_err(PdfConvertError::from)?;

            let response = handle_response(response, "Docling file conversion").await?;
            response.json::<Value>().await.map_err(|e| {
                PdfConvertError::parse_error("Docling conversion response", e.to_string())
            })
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
            let url = format!("{}/convert/file/async", self.config.base_url);
            let response = self
                .http_client
                .post(&url)
                .multipart(form)
                .send()
                .await
                .map_err(PdfConvertError::from)?;

            let response = handle_response(response, "Docling async submission").await?;
            let result = response.json::<TaskPostResponse>().await.map_err(|e| {
                PdfConvertError::parse_error("Docling async submission response", e.to_string())
            })?;
            Ok(result.task_id)
        };

        retry_with_backoff(operation, "docling_submit_file_async").await
    }

    pub async fn wait_for_result(&self, task_id: &str) -> Result<Value> {
        loop {
            match self.check_task_status(task_id).await? {
                true => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    return self.get_task_result(task_id, true).await;
                }
                false => tokio::time::sleep(Duration::from_secs(5)).await,
            }
        }
    }

    pub async fn check_task_status(&self, task_id: &str) -> Result<bool> {
        let operation = || async {
            let url = format!("{}/status/poll/{}", self.config.base_url, task_id);
            let response = get_request(&self.http_client, &url, "Polling task status").await?;
            let response_text = response.text().await?;

            match serde_json::from_str::<TaskStatusResponse>(&response_text) {
                Ok(status_response) => match status_response.task_status.as_str() {
                    "success" => Ok(true),
                    "failure" | "revoked" => {
                        let error_details = extract_error_details(&response_text);
                        Err(PdfConvertError::api_task_failed(
                            status_response.task_status,
                            error_details,
                        ))
                    }
                    _ => Ok(false),
                },
                Err(_) => Err(PdfConvertError::parse_error(
                    "task status response",
                    format!(
                        "Task {} returned invalid response: {}",
                        task_id, response_text
                    ),
                )),
            }
        };

        retry_with_backoff(operation, &format!("check_task_status({task_id})")).await
    }

    pub async fn get_task_result(&self, task_id: &str, use_new_conn: bool) -> Result<Value> {
        let operation = || async {
            let url = format!("{}/result/{}", self.config.base_url, task_id);
            let response = get_request_with_conn_close(
                &self.http_client,
                &url,
                "Fetching task result",
                use_new_conn,
            )
            .await?;

            response
                .json::<Value>()
                .await
                .map_err(|e| PdfConvertError::parse_error("task result response", e.to_string()))
        };

        retry_with_backoff(operation, &format!("get_task_result({task_id})")).await
    }

    fn build_form(
        &self,
        input: &InputDocument,
        request: &DoclingConvertRequest,
    ) -> Result<multipart::Form> {
        let input_kind = input.kind()?;
        let part = multipart::Part::stream(reqwest::Body::from(Bytes::clone(&input.bytes)))
            .file_name(input.filename.clone())
            .mime_str(&input.media_type)
            .map_err(|e| {
                PdfConvertError::api_error(None, format!("Failed to create multipart part: {e}"))
            })?;

        let mut form = multipart::Form::new()
            .part("files", part)
            .text("from_formats", input_kind.from_formats_value().to_string())
            .text("target_type", "inbody");

        for format in &request.output_formats {
            form = form.text("to_formats", format.as_api_value().to_string());
        }

        if let Some((start_page, end_page)) = request.page_range {
            form = form.text("page_range", start_page.to_string());
            form = form.text("page_range", end_page.to_string());
        }

        if request.chunking {
            form = form.text("include_chunking", "true");
        }

        if input_kind.supports_vlm()
            && let Some(vlm_config) = self.config.resolved_vlm_config()?
        {
            form = self.apply_vlm_config(form, &vlm_config)?;
        }

        Ok(form)
    }

    fn apply_vlm_config(
        &self,
        mut form: multipart::Form,
        vlm_config: &ResolvedVlmConfig,
    ) -> Result<multipart::Form> {
        let picture_description_custom_config =
            PictureDescriptionVlmEngineOptions::for_openai_compatible(
                &vlm_config.openai_base_url,
                &vlm_config.api_key,
                &vlm_config.picture_description_model,
                "Describe this image in a few sentences.",
                300,
                60,
            );
        let code_formula_custom_config = CodeFormulaVlmOptions {
            scale: Some(2.0),
            max_size: None,
            extract_code: Some(true),
            extract_formulas: Some(true),
            engine_options: OpenRouterConfigBuilder::engine_options(
                &vlm_config.openai_base_url,
                &vlm_config.api_key,
                &vlm_config.code_formula_model,
                30,
                2,
            ),
            model_spec: OpenRouterConfigBuilder::model_spec(
                &vlm_config.code_formula_model,
                "Recognize code blocks and mathematical formulas in the image. For code, output the full code; for mathematical formulas, output in LaTeX format.",
                1000,
            ),
        };
        let vlm_pipeline_custom_config = VlmConvertOptions {
            engine_options: OpenRouterConfigBuilder::engine_options(
                &vlm_config.openai_base_url,
                &vlm_config.api_key,
                &vlm_config.vlm_pipeline_model,
                30,
                2,
            ),
            model_spec: OpenRouterConfigBuilder::model_spec(
                &vlm_config.vlm_pipeline_model,
                "",
                1000,
            ),
            scale: Some(1.0),
            max_size: None,
            batch_size: None,
            force_backend_text: true,
        };

        form = form.text(
            "vlm_pipeline_custom_config",
            serde_json::to_string(&vlm_pipeline_custom_config)?,
        );
        form = form.text(
            "picture_description_custom_config",
            serde_json::to_string(&picture_description_custom_config)?,
        );
        form = form.text(
            "code_formula_custom_config",
            serde_json::to_string(&code_formula_custom_config)?,
        );
        form = form.text("do_code_enrichment", "True");
        form = form.text("do_formula_enrichment", "True");
        form = form.text("do_picture_description", "True");
        form = form.text("ocr_engine", "rapidocr");
        form = form.text("image_export_mode", "placeholder");

        Ok(form)
    }
}

#[cfg(test)]
#[path = "docling_tests.rs"]
mod tests;
