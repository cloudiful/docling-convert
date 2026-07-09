use bytes::Bytes;

use crate::api::DoclingConfig;
use crate::conversion::{
    ConversionBehavior, DoclingRuntimeConfig, build_convert_options, build_docling_client,
};
use crate::document::{ConvertRequest, ConvertedDocument, InputDocument, InputKind, OutputFormat};
use crate::error::{PdfConvertError, Result};
use crate::processor::DocumentConverter;

pub struct ConverterBuilder {
    config: DoclingRuntimeConfig,
    behavior: ConversionBehavior,
    output_formats: Vec<OutputFormat>,
}

impl ConverterBuilder {
    pub fn new(config: DoclingRuntimeConfig) -> Self {
        Self {
            config,
            behavior: ConversionBehavior::default(),
            output_formats: vec![OutputFormat::Md],
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

    pub fn build(self) -> Result<PdfConvert> {
        let output_formats = if self.output_formats.is_empty() {
            vec![OutputFormat::Md]
        } else {
            self.output_formats
        };

        Ok(PdfConvert {
            converter: DocumentConverter::new(build_docling_client(self.config)?),
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
}
