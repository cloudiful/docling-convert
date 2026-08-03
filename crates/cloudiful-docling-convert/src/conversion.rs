use std::time::Duration;

use crate::api::{DoclingClient, DoclingConfig};
use crate::document::{
    ChunkerKind, ChunkingOptions, ConvertOptions, InputKind, PipelineKind, RemoteConvertOptions,
    TextConvertOptions,
};
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct DoclingRuntimeConfig {
    pub docling_base_url: String,
    pub openai_base_url: String,
    pub vlm_pipeline_model: String,
    pub picture_description_model: String,
    pub code_formula_model: String,
    pub api_key: Option<String>,
    pub tenant_id: Option<String>,
    pub openai_api_key: Option<String>,
    pub request_timeout: Option<Duration>,
}

impl DoclingRuntimeConfig {
    pub fn without_vlm(docling_base_url: impl Into<String>) -> Self {
        Self {
            docling_base_url: docling_base_url.into(),
            openai_base_url: String::new(),
            vlm_pipeline_model: String::new(),
            picture_description_model: String::new(),
            code_formula_model: String::new(),
            api_key: None,
            tenant_id: None,
            openai_api_key: None,
            request_timeout: None,
        }
    }

    pub fn from_env(docling_base_url: impl Into<String>) -> Self {
        let mut config = Self::without_vlm(docling_base_url);
        config.api_key = std::env::var("DOCLING_API_KEY").ok();
        config.tenant_id = std::env::var("DOCLING_TENANT_ID").ok();
        config.openai_api_key = std::env::var("OPENAI_API_KEY").ok();
        config.request_timeout = std::env::var("DOCLING_HTTP_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs);
        config
    }

    pub fn into_docling_config(self) -> DoclingConfig {
        DoclingConfig {
            base_url: self.docling_base_url,
            openai_base_url: self.openai_base_url,
            vlm_pipeline_model: self.vlm_pipeline_model,
            picture_description_model: self.picture_description_model,
            code_formula_model: self.code_formula_model,
            api_key: self
                .api_key
                .or_else(|| std::env::var("DOCLING_API_KEY").ok()),
            openai_api_key: self
                .openai_api_key
                .or_else(|| std::env::var("OPENAI_API_KEY").ok()),
            tenant_id: self
                .tenant_id
                .or_else(|| std::env::var("DOCLING_TENANT_ID").ok()),
            request_timeout: self.request_timeout,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionBehavior {
    pub chunker: ChunkerKind,
    pub chunking: ChunkingOptions,
    pub pipeline: Option<PipelineKind>,
}

impl Default for ConversionBehavior {
    fn default() -> Self {
        Self {
            chunker: ChunkerKind::None,
            chunking: ChunkingOptions::hybrid_defaults(),
            pipeline: None,
        }
    }
}

pub fn build_docling_client(config: DoclingRuntimeConfig) -> Result<DoclingClient> {
    DoclingClient::new(config.into_docling_config())
}

pub fn build_convert_options(
    input_kind: InputKind,
    behavior: &ConversionBehavior,
) -> Result<ConvertOptions> {
    let remote_options = RemoteConvertOptions {
        chunker: behavior.chunker,
        chunking: behavior.chunking.clone(),
        pipeline: behavior.pipeline,
    };

    if matches!(input_kind, InputKind::Pdf) {
        Ok(ConvertOptions::Pdf(remote_options))
    } else if input_kind.uses_generic_convert_options() {
        Ok(ConvertOptions::Generic(remote_options))
    } else {
        Ok(ConvertOptions::Text(TextConvertOptions::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_behavior_uses_normal_conversion() {
        assert_eq!(ConversionBehavior::default().chunker, ChunkerKind::None);
    }

    #[test]
    fn generic_inputs_use_native_chunker_options() {
        let options = build_convert_options(
            InputKind::Docx,
            &ConversionBehavior {
                chunker: ChunkerKind::Hierarchical,
                ..ConversionBehavior::default()
            },
        )
        .unwrap();

        assert!(matches!(options, ConvertOptions::Generic(_)));
    }

    #[test]
    fn text_inputs_remain_local() {
        let options =
            build_convert_options(InputKind::Text, &ConversionBehavior::default()).unwrap();
        assert!(matches!(options, ConvertOptions::Text(_)));
    }

    #[test]
    fn runtime_config_keeps_docling_and_openai_keys_separate() {
        let config =
            DoclingRuntimeConfig::without_vlm("http://localhost:5001/v1").into_docling_config();
        assert!(config.api_key.is_none());
        assert!(config.openai_api_key.is_none());
    }
}
