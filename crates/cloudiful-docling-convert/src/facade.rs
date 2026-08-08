use bytes::Bytes;

use crate::api::DoclingConfig;
use crate::conversion::{
    ConversionBehavior, DoclingRuntimeConfig, build_convert_options, build_docling_client,
};
use crate::document::{ConvertRequest, ConvertedDocument, InputDocument, InputKind, OutputFormat};
use crate::error::{PdfConvertError, Result};
use crate::models::TaskStatusResponse;
use crate::processor::DocumentConverter;

pub struct ConverterBuilder {
    config: DoclingRuntimeConfig,
    behavior: ConversionBehavior,
    output_formats: Vec<OutputFormat>,
    result_body_limit: Option<usize>,
}

impl ConverterBuilder {
    pub fn new(config: DoclingRuntimeConfig) -> Self {
        Self {
            config,
            behavior: ConversionBehavior::default(),
            output_formats: vec![OutputFormat::Md],
            result_body_limit: None,
        }
    }

    pub fn behavior(mut self, behavior: ConversionBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    pub fn output_formats(mut self, output_formats: Vec<OutputFormat>) -> Self {
        self.output_formats = output_formats;
        self
    }

    pub fn result_body_limit(mut self, result_body_limit: usize) -> Self {
        self.result_body_limit = Some(result_body_limit);
        self
    }

    pub fn build(self) -> Result<PdfConvert> {
        let output_formats = if self.output_formats.is_empty() {
            vec![OutputFormat::Md]
        } else {
            self.output_formats
        };

        let client = match self.result_body_limit {
            Some(limit) => crate::DoclingClient::new_with_result_body_limit(
                self.config.into_docling_config(),
                limit,
            )?,
            None => build_docling_client(self.config)?,
        };
        Ok(PdfConvert {
            converter: DocumentConverter::new(client),
            behavior: self.behavior,
            output_formats,
        })
    }
}

pub struct PdfConvert {
    converter: DocumentConverter,
    behavior: ConversionBehavior,
    output_formats: Vec<OutputFormat>,
}

impl PdfConvert {
    pub fn builder(config: DoclingRuntimeConfig) -> ConverterBuilder {
        ConverterBuilder::new(config)
    }

    pub fn from_runtime_config(config: DoclingRuntimeConfig) -> Result<Self> {
        Self::builder(config).build()
    }

    pub fn from_docling_config(config: DoclingConfig) -> Result<Self> {
        Ok(Self {
            converter: DocumentConverter::new(crate::DoclingClient::new(config)?),
            behavior: ConversionBehavior::default(),
            output_formats: vec![OutputFormat::Md],
        })
    }

    pub fn request_for_input(&self, input: InputDocument) -> Result<ConvertRequest> {
        let input_kind = input.kind()?;

        Ok(ConvertRequest {
            input,
            output_formats: self.output_formats.clone(),
            options: build_convert_options(input_kind, &self.behavior)?,
        })
    }

    pub async fn convert_input(&self, input: InputDocument) -> Result<ConvertedDocument> {
        self.converter.convert(self.request_for_input(input)?).await
    }

    pub async fn convert_input_async(&self, input: InputDocument) -> Result<ConvertedDocument> {
        self.converter
            .convert_async(self.request_for_input(input)?)
            .await
    }

    /// Submit a whole-document asynchronous conversion and return a resumable
    /// [`DoclingTaskHandle`]. Unlike [`Self::convert_input_async`], the remote
    /// task id is returned to the caller so polling can be paused and resumed,
    /// even across process restarts.
    pub async fn submit_async(&self, input: InputDocument) -> Result<DoclingTaskHandle> {
        let request = self.request_for_input(input)?;
        let task_id = self.converter.submit_async(&request).await?;
        Ok(DoclingTaskHandle {
            converter: self.converter.clone(),
            input: request.input,
            task_id,
        })
    }

    /// Poll a remote task by its id. Uses Docling's long-polling endpoint.
    pub async fn poll_remote(&self, task_id: &str) -> Result<TaskStatusResponse> {
        self.converter
            .docling_client
            .poll_task_status(task_id)
            .await
    }

    /// Fetch and parse a completed remote task by its id. The task must have
    /// reached a terminal status; poll with [`Self::poll_remote`] first.
    pub async fn fetch_remote(
        &self,
        input: InputDocument,
        task_id: &str,
    ) -> Result<ConvertedDocument> {
        let status = self
            .converter
            .docling_client
            .poll_task_status(task_id)
            .await?;
        let task_result = self
            .converter
            .docling_client
            .fetch_task_result(task_id, &status)
            .await?;
        DocumentConverter::document_from_task_result(&input, task_result)
    }

    pub async fn convert_bytes(
        &self,
        filename: impl Into<String>,
        bytes: impl Into<Bytes>,
    ) -> Result<ConvertedDocument> {
        let filename = filename.into();
        let input_kind =
            InputKind::from_filename_and_media_type(&filename, None).ok_or_else(|| {
                PdfConvertError::validation_error(
                    "filename",
                    format!("unsupported input type for '{}'", filename),
                )
            })?;

        self.convert_input(InputDocument::new(
            filename.clone(),
            input_kind.canonical_media_type(&filename, None),
            bytes,
        ))
        .await
    }

    pub async fn convert_bytes_with_input_kind(
        &self,
        filename: impl Into<String>,
        bytes: impl Into<Bytes>,
        input_kind: InputKind,
    ) -> Result<ConvertedDocument> {
        let filename = filename.into();

        self.convert_input(
            InputDocument::new(
                filename.clone(),
                input_kind.canonical_media_type(&filename, None),
                bytes,
            )
            .with_input_kind(input_kind),
        )
        .await
    }
}

/// A resumable handle to a whole-document asynchronous Docling conversion.
///
/// The caller controls the polling cadence and deadline; the underlying remote
/// task keeps running in Docling while this handle is idle, so the handle can
/// be serialized by its [`task_id`](Self::task_id) and resumed after a restart
/// by submitting a new handle for the same remote task through the client API.
#[derive(Clone)]
pub struct DoclingTaskHandle {
    converter: DocumentConverter,
    input: InputDocument,
    task_id: String,
}

impl DoclingTaskHandle {
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Poll the remote task status. Uses Docling's long-polling endpoint, which
    /// blocks server-side for a bounded period before returning the status.
    pub async fn poll_status(&self) -> Result<TaskStatusResponse> {
        self.converter
            .docling_client
            .poll_task_status(&self.task_id)
            .await
    }

    /// Fetch and parse the completed remote result. Requires the task to have
    /// reached a terminal status; call [`Self::poll_status`] first.
    pub async fn fetch_result(&self) -> Result<ConvertedDocument> {
        let status = self
            .converter
            .docling_client
            .poll_task_status(&self.task_id)
            .await?;
        let task_result = self
            .converter
            .docling_client
            .fetch_task_result(&self.task_id, &status)
            .await?;
        DocumentConverter::document_from_task_result(&self.input, task_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults_to_markdown_output() {
        let converter = ConverterBuilder::new(DoclingRuntimeConfig {
            docling_base_url: "http://127.0.0.1:5001/v1".into(),
            openai_base_url: "https://example.com/v1".into(),
            vlm_pipeline_model: "test-model".into(),
            picture_description_model: "test-model".into(),
            code_formula_model: "test-model".into(),
            api_key: Some("key".into()),
            ..DoclingRuntimeConfig::without_vlm("http://127.0.0.1:5001/v1")
        })
        .build()
        .unwrap();

        let request = converter
            .request_for_input(InputDocument::new(
                "notes.md",
                "text/markdown",
                Bytes::from("# hi"),
            ))
            .unwrap();

        assert_eq!(request.output_formats, vec![OutputFormat::Md]);
    }

    #[test]
    fn convert_bytes_rejects_unknown_extensions() {
        let converter = ConverterBuilder::new(DoclingRuntimeConfig {
            docling_base_url: "http://127.0.0.1:5001/v1".into(),
            openai_base_url: "https://example.com/v1".into(),
            vlm_pipeline_model: "test-model".into(),
            picture_description_model: "test-model".into(),
            code_formula_model: "test-model".into(),
            api_key: Some("key".into()),
            ..DoclingRuntimeConfig::without_vlm("http://127.0.0.1:5001/v1")
        })
        .build()
        .unwrap();

        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(converter.convert_bytes("notes.bin", Bytes::from_static(b"test")))
            .unwrap_err();

        assert!(error.to_string().contains("unsupported input type"));
    }

    #[test]
    fn convert_bytes_with_input_kind_accepts_ambiguous_sources() {
        let converter = ConverterBuilder::new(DoclingRuntimeConfig {
            docling_base_url: "http://127.0.0.1:5001/v1".into(),
            openai_base_url: "https://example.com/v1".into(),
            vlm_pipeline_model: "test-model".into(),
            picture_description_model: "test-model".into(),
            code_formula_model: "test-model".into(),
            api_key: Some("key".into()),
            ..DoclingRuntimeConfig::without_vlm("http://127.0.0.1:5001/v1")
        })
        .build()
        .unwrap();

        let request = converter
            .request_for_input(
                InputDocument::new("paper.xml", "application/xml", Bytes::from("<article />"))
                    .with_input_kind(InputKind::XmlJats),
            )
            .unwrap();

        assert_eq!(request.input.kind().unwrap(), InputKind::XmlJats);
    }

    #[test]
    fn builder_result_body_limit_zero_fails_build() {
        let error = match ConverterBuilder::new(DoclingRuntimeConfig {
            docling_base_url: "http://127.0.0.1:5001/v1".into(),
            openai_base_url: "https://example.com/v1".into(),
            vlm_pipeline_model: "test-model".into(),
            picture_description_model: "test-model".into(),
            code_formula_model: "test-model".into(),
            api_key: Some("key".into()),
            ..DoclingRuntimeConfig::without_vlm("http://127.0.0.1:5001/v1")
        })
        .result_body_limit(0)
        .build()
        {
            Ok(_) => panic!("build should fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("result_body_limit"));
        assert!(error.to_string().contains("greater than 0"));
    }

    #[test]
    fn builder_result_body_limit_builds_with_default_outputs() {
        let converter = ConverterBuilder::new(DoclingRuntimeConfig {
            docling_base_url: "http://127.0.0.1:5001/v1".into(),
            openai_base_url: "https://example.com/v1".into(),
            vlm_pipeline_model: "test-model".into(),
            picture_description_model: "test-model".into(),
            code_formula_model: "test-model".into(),
            api_key: Some("key".into()),
            ..DoclingRuntimeConfig::without_vlm("http://127.0.0.1:5001/v1")
        })
        .result_body_limit(1024)
        .build()
        .unwrap();

        let request = converter
            .request_for_input(InputDocument::new(
                "notes.md",
                "text/markdown",
                Bytes::from("# hi"),
            ))
            .unwrap();

        assert_eq!(request.output_formats, vec![OutputFormat::Md]);
    }
}
