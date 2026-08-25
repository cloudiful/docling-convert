use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::document::{ChunkerKind, ChunkingOptions, InputKind, PipelineKind};

use super::docling::DoclingConvertRequest;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum HttpSource {
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, Value>,
    },
}

impl HttpSource {
    pub fn new(url: impl Into<String>) -> Self {
        Self::Http {
            url: url.into(),
            headers: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum TargetKind {
    #[serde(rename = "inbody")]
    Inbody {},
    #[serde(rename = "zip")]
    Zip {},
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ConvertDocumentsOptions {
    pub from_formats: Vec<String>,
    pub to_formats: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_range: Option<[u64; 2]>,
    /// Preset ID for picture description. When set, Docling Serve uses the
    /// admin-defined preset and ignores any `picture_description_custom_config`
    /// the legacy VLM bundle may emit on the multipart convert endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture_description_preset: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ConvertSourcesRequest {
    pub options: ConvertDocumentsOptions,
    pub sources: Vec<HttpSource>,
    pub target: TargetKind,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChunkSourcesRequest {
    pub convert_options: ConvertDocumentsOptions,
    pub sources: Vec<HttpSource>,
    pub include_converted_doc: bool,
    pub target: TargetKind,
    pub chunking_options: ChunkingRequestOptions,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "chunker")]
pub(crate) enum ChunkingRequestOptions {
    #[serde(rename = "hybrid")]
    Hybrid {
        use_markdown_tables: bool,
        use_markdown_images: bool,
        image_placeholder: String,
        include_raw_text: bool,
        max_tokens: Option<u32>,
        tokenizer: String,
        merge_peers: bool,
    },
    #[serde(rename = "hierarchical")]
    Hierarchical {
        use_markdown_tables: bool,
        use_markdown_images: bool,
        image_placeholder: String,
        include_raw_text: bool,
    },
}

pub(crate) fn convert_options(
    input_kind: InputKind,
    request: &DoclingConvertRequest,
) -> ConvertDocumentsOptions {
    ConvertDocumentsOptions {
        from_formats: vec![input_kind.from_formats_value().to_string()],
        to_formats: request
            .output_formats
            .iter()
            .map(|format| format.as_api_value().to_string())
            .collect(),
        pipeline: request.pipeline.map(|pipeline| pipeline.to_string()),
        page_range: request
            .page_range
            .map(|(start, end)| [start as u64, end as u64]),
        picture_description_preset: request.picture_description_preset.clone(),
    }
}

pub(crate) fn target_for(request: &DoclingConvertRequest) -> TargetKind {
    if request
        .output_formats
        .iter()
        .any(|format| format.is_archive())
    {
        TargetKind::Zip {}
    } else {
        TargetKind::Inbody {}
    }
}

pub(crate) fn chunking_options(
    chunker: ChunkerKind,
    options: &ChunkingOptions,
) -> crate::error::Result<ChunkingRequestOptions> {
    match chunker {
        ChunkerKind::Hybrid => {
            let defaults = ChunkingOptions::hybrid_defaults();
            Ok(ChunkingRequestOptions::Hybrid {
                use_markdown_tables: options.use_markdown_tables,
                use_markdown_images: options.use_markdown_images,
                image_placeholder: if options.image_placeholder.is_empty() {
                    defaults.image_placeholder
                } else {
                    options.image_placeholder.clone()
                },
                include_raw_text: options.include_raw_text,
                max_tokens: options.max_tokens,
                tokenizer: options
                    .tokenizer
                    .clone()
                    .unwrap_or_else(|| defaults.tokenizer.unwrap_or_default()),
                merge_peers: options.merge_peers,
            })
        }
        ChunkerKind::Hierarchical => Ok(ChunkingRequestOptions::Hierarchical {
            use_markdown_tables: options.use_markdown_tables,
            use_markdown_images: options.use_markdown_images,
            image_placeholder: if options.image_placeholder.is_empty() {
                ChunkingOptions::hierarchical_defaults().image_placeholder
            } else {
                options.image_placeholder.clone()
            },
            include_raw_text: options.include_raw_text,
        }),
        ChunkerKind::None => Err(crate::error::PdfConvertError::validation_error(
            "chunker",
            "chunking options require hybrid or hierarchical",
        )),
    }
}

pub(crate) fn source_request(
    url: impl Into<String>,
    input_kind: InputKind,
    request: &DoclingConvertRequest,
) -> ConvertSourcesRequest {
    ConvertSourcesRequest {
        options: convert_options(input_kind, request),
        sources: vec![HttpSource::new(url)],
        target: target_for(request),
    }
}

pub(crate) fn chunk_source_request(
    url: impl Into<String>,
    input_kind: InputKind,
    request: &DoclingConvertRequest,
) -> crate::error::Result<ChunkSourcesRequest> {
    Ok(ChunkSourcesRequest {
        convert_options: convert_options(input_kind, request),
        sources: vec![HttpSource::new(url)],
        include_converted_doc: false,
        target: target_for(request),
        chunking_options: chunking_options(request.chunker, &request.chunking)?,
    })
}

impl std::fmt::Display for PipelineKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            PipelineKind::Legacy => "legacy",
            PipelineKind::Standard => "standard",
            PipelineKind::Vlm => "vlm",
            PipelineKind::Asr => "asr",
        };
        f.write_str(value)
    }
}
