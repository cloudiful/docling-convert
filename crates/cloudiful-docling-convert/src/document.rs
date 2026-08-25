use std::path::Path;
use std::str::FromStr;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::error::{PdfConvertError, Result};
use crate::models::{ChunkDocumentResponse, DoclingChunk};

mod input_kind;

pub use input_kind::InputKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Json,
    Md,
    Yaml,
    Html,
    HtmlSplitPage,
    Text,
    Doctags,
    Vtt,
    Doclang,
    Dclx,
    Chunks,
}

impl OutputFormat {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Md => "md",
            Self::Yaml => "yaml",
            Self::Html => "html",
            Self::HtmlSplitPage => "html_split_page",
            Self::Text => "text",
            Self::Doctags => "doctags",
            Self::Vtt => "vtt",
            Self::Doclang => "doclang",
            Self::Dclx => "dclx",
            Self::Chunks => "chunks",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Chunks => "chunks.json",
            Self::Yaml | Self::HtmlSplitPage | Self::Vtt | Self::Dclx => "zip",
            _ => self.as_api_value(),
        }
    }

    pub fn is_archive(self) -> bool {
        matches!(
            self,
            Self::Yaml | Self::HtmlSplitPage | Self::Vtt | Self::Dclx
        )
    }

    pub fn is_chunk_output(self) -> bool {
        matches!(self, Self::Chunks)
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_api_value())
    }
}

impl FromStr for OutputFormat {
    type Err = PdfConvertError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "md" | "markdown" => Ok(Self::Md),
            "yaml" | "yml" => Ok(Self::Yaml),
            "html" => Ok(Self::Html),
            "html_split_page" | "html-split-page" => Ok(Self::HtmlSplitPage),
            "text" | "txt" => Ok(Self::Text),
            "doctags" => Ok(Self::Doctags),
            "vtt" => Ok(Self::Vtt),
            "doclang" => Ok(Self::Doclang),
            "dclx" => Ok(Self::Dclx),
            "chunks" => Ok(Self::Chunks),
            other => Err(PdfConvertError::validation_error(
                "format",
                format!("unsupported output format: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChunkerKind {
    #[default]
    None,
    Hybrid,
    Hierarchical,
}

impl ChunkerKind {
    pub fn is_enabled(self) -> bool {
        !matches!(self, Self::None)
    }
}

impl std::fmt::Display for ChunkerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::None => "none",
            Self::Hybrid => "hybrid",
            Self::Hierarchical => "hierarchical",
        };
        f.write_str(value)
    }
}

impl FromStr for ChunkerKind {
    type Err = PdfConvertError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" | "" => Ok(Self::None),
            "hybrid" => Ok(Self::Hybrid),
            "hierarchical" => Ok(Self::Hierarchical),
            other => Err(PdfConvertError::validation_error(
                "chunker",
                format!("unsupported chunker: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChunkingOptions {
    pub use_markdown_tables: bool,
    pub use_markdown_images: bool,
    pub image_placeholder: String,
    pub include_raw_text: bool,
    pub max_tokens: Option<u32>,
    pub tokenizer: Option<String>,
    pub merge_peers: bool,
}

impl ChunkingOptions {
    pub fn hybrid_defaults() -> Self {
        Self {
            image_placeholder: "![IMAGE]".to_string(),
            tokenizer: Some("sentence-transformers/all-MiniLM-L6-v2".to_string()),
            merge_peers: true,
            ..Self::default()
        }
    }

    pub fn hierarchical_defaults() -> Self {
        Self {
            image_placeholder: "![IMAGE]".to_string(),
            merge_peers: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PipelineKind {
    Legacy,
    Standard,
    Vlm,
    Asr,
}

impl FromStr for PipelineKind {
    type Err = PdfConvertError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "legacy" => Ok(Self::Legacy),
            "standard" => Ok(Self::Standard),
            "vlm" => Ok(Self::Vlm),
            "asr" => Ok(Self::Asr),
            other => Err(PdfConvertError::validation_error(
                "pipeline",
                format!("unsupported pipeline: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputDocument {
    pub filename: String,
    pub media_type: String,
    pub bytes: Bytes,
    pub input_kind_override: Option<InputKind>,
}

impl InputDocument {
    pub fn new(
        filename: impl Into<String>,
        media_type: impl Into<String>,
        bytes: impl Into<Bytes>,
    ) -> Self {
        Self {
            filename: filename.into(),
            media_type: media_type.into(),
            bytes: bytes.into(),
            input_kind_override: None,
        }
    }

    pub fn with_input_kind(mut self, input_kind: InputKind) -> Self {
        self.input_kind_override = Some(input_kind);
        self
    }

    pub fn from_path_and_bytes(path: &Path, bytes: impl Into<Bytes>) -> Result<Self> {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                PdfConvertError::validation_error(
                    "input_path",
                    format!("path '{}' does not have a valid file name", path.display()),
                )
            })?;
        let kind = InputKind::from_path(path).ok_or_else(|| {
            PdfConvertError::validation_error(
                "input_path",
                format!("unsupported file type for '{}'", path.display()),
            )
        })?;

        Ok(Self::new(
            filename,
            kind.canonical_media_type(filename, None),
            bytes,
        ))
    }

    pub fn from_path_and_bytes_with_kind(
        path: &Path,
        bytes: impl Into<Bytes>,
        input_kind: InputKind,
    ) -> Result<Self> {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                PdfConvertError::validation_error(
                    "input_path",
                    format!("path '{}' does not have a valid file name", path.display()),
                )
            })?;

        Ok(Self::new(
            filename,
            input_kind.canonical_media_type(filename, None),
            bytes,
        )
        .with_input_kind(input_kind))
    }

    pub fn kind(&self) -> Result<InputKind> {
        if let Some(input_kind) = self.input_kind_override {
            return Ok(input_kind);
        }

        InputKind::from_filename_and_media_type(&self.filename, Some(&self.media_type)).ok_or_else(
            || {
                let reason = if InputKind::requires_explicit_override(
                    &self.filename,
                    Some(&self.media_type),
                ) {
                    format!(
                        "ambiguous input type for '{}' ({}); provide an explicit input_format override",
                        self.filename, self.media_type
                    )
                } else {
                    format!(
                        "unsupported input type for '{}' ({})",
                        self.filename, self.media_type
                    )
                };
                PdfConvertError::validation_error("input", reason)
            },
        )
    }
}

#[derive(Debug, Clone)]
pub struct ConvertRequest {
    pub input: InputDocument,
    pub output_formats: Vec<OutputFormat>,
    pub options: ConvertOptions,
}

impl ConvertRequest {
    pub fn validate(&self) -> Result<InputKind> {
        let kind = self.input.kind()?;
        match (&self.options, kind) {
            (ConvertOptions::Pdf(_), InputKind::Pdf)
            | (ConvertOptions::Text(_), InputKind::Text) => {}
            (ConvertOptions::Generic(_), _) if kind.uses_generic_convert_options() => {}
            (_, InputKind::Pdf) => {
                return Err(PdfConvertError::validation_error(
                    "options",
                    "PDF input requires PdfConvertOptions",
                ));
            }
            (_, InputKind::Text) => {
                return Err(PdfConvertError::validation_error(
                    "options",
                    "txt input requires TextConvertOptions",
                ));
            }
            _ => {
                return Err(PdfConvertError::validation_error(
                    "options",
                    "non-pdf, non-text input requires GenericFileConvertOptions",
                ));
            }
        }

        if self.output_formats.is_empty() {
            return Err(PdfConvertError::validation_error(
                "output_formats",
                "at least one output format is required",
            ));
        }

        let chunker = match &self.options {
            ConvertOptions::Pdf(options) | ConvertOptions::Generic(options) => options.chunker,
            ConvertOptions::Text(_) => ChunkerKind::None,
        };

        if chunker.is_enabled() && self.output_formats.iter().any(|format| format.is_archive()) {
            return Err(PdfConvertError::validation_error(
                "output_formats",
                "native chunking cannot be combined with archive outputs",
            ));
        }

        if self
            .output_formats
            .iter()
            .any(|format| format.is_chunk_output())
            && !chunker.is_enabled()
        {
            return Err(PdfConvertError::validation_error(
                "chunker",
                "chunks output requires hybrid or hierarchical chunking",
            ));
        }

        Ok(kind)
    }
}

#[derive(Debug, Clone)]
pub enum ConvertOptions {
    Pdf(PdfConvertOptions),
    Generic(GenericFileConvertOptions),
    Text(TextConvertOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteConvertOptions {
    pub chunker: ChunkerKind,
    pub chunking: ChunkingOptions,
    pub pipeline: Option<PipelineKind>,
    /// Preset ID for picture description, forwarded to Docling Serve as
    /// `picture_description_preset`. Mutually exclusive with the legacy
    /// `picture_description_custom_config` emitted when a VLM bundle is
    /// configured; leaving this `None` preserves the legacy custom VLM
    /// behaviour.
    pub picture_description_preset: Option<String>,
}

impl Default for RemoteConvertOptions {
    fn default() -> Self {
        Self {
            chunker: ChunkerKind::None,
            chunking: ChunkingOptions::hybrid_defaults(),
            pipeline: None,
            picture_description_preset: None,
        }
    }
}

pub type PdfConvertOptions = RemoteConvertOptions;
pub type GenericFileConvertOptions = RemoteConvertOptions;

#[derive(Debug, Clone)]
pub struct TextConvertOptions {
    pub normalize_line_endings: bool,
    pub trim_utf8_bom: bool,
}

impl Default for TextConvertOptions {
    fn default() -> Self {
        Self {
            normalize_line_endings: true,
            trim_utf8_bom: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertedDocumentMetadata {
    pub input_kind: InputKind,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertedDocument {
    pub filename: String,
    pub markdown: Option<String>,
    pub text: Option<String>,
    pub json: Option<serde_json::Value>,
    pub html: Option<String>,
    pub doctags: Option<String>,
    pub doclang: Option<String>,
    pub chunks: Vec<DoclingChunk>,
    pub chunk_response: Option<ChunkDocumentResponse>,
    #[serde(skip)]
    pub archive: Option<Vec<u8>>,
    pub metadata: ConvertedDocumentMetadata,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileConvertRequest {
    pub request: ConvertRequest,
    pub output_dir: std::path::PathBuf,
    pub selected_output: OutputFormat,
    pub overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct ConvertedFile {
    pub document: ConvertedDocument,
    pub output_paths: Vec<std::path::PathBuf>,
}

pub fn supported_input_kind(path: &Path) -> bool {
    InputKind::from_path(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_supports_native_and_archive_outputs() {
        for (value, format) in [
            ("md", OutputFormat::Md),
            ("markdown", OutputFormat::Md),
            ("json", OutputFormat::Json),
            ("yaml", OutputFormat::Yaml),
            ("yml", OutputFormat::Yaml),
            ("html", OutputFormat::Html),
            ("html_split_page", OutputFormat::HtmlSplitPage),
            ("html-split-page", OutputFormat::HtmlSplitPage),
            ("text", OutputFormat::Text),
            ("txt", OutputFormat::Text),
            ("doctags", OutputFormat::Doctags),
            ("vtt", OutputFormat::Vtt),
            ("doclang", OutputFormat::Doclang),
            ("dclx", OutputFormat::Dclx),
            ("chunks", OutputFormat::Chunks),
        ] {
            assert_eq!(value.parse::<OutputFormat>().unwrap(), format);
        }

        assert_eq!(OutputFormat::Chunks.extension(), "chunks.json");
        for format in [
            OutputFormat::Yaml,
            OutputFormat::HtmlSplitPage,
            OutputFormat::Vtt,
            OutputFormat::Dclx,
        ] {
            assert_eq!(format.extension(), "zip");
            assert!(format.is_archive());
        }
    }

    #[test]
    fn chunking_rejects_archive_outputs() {
        let request = ConvertRequest {
            input: InputDocument::new("a.pdf", "application/pdf", Bytes::from_static(b"%PDF")),
            output_formats: vec![OutputFormat::Yaml],
            options: ConvertOptions::Pdf(RemoteConvertOptions {
                chunker: ChunkerKind::Hybrid,
                ..RemoteConvertOptions::default()
            }),
        };

        assert!(
            request
                .validate()
                .unwrap_err()
                .to_string()
                .contains("archive")
        );
    }

    #[test]
    fn chunks_require_a_native_chunker() {
        let request = ConvertRequest {
            input: InputDocument::new("a.pdf", "application/pdf", Bytes::from_static(b"%PDF")),
            output_formats: vec![OutputFormat::Chunks],
            options: ConvertOptions::Pdf(RemoteConvertOptions::default()),
        };

        assert!(
            request
                .validate()
                .unwrap_err()
                .to_string()
                .contains("requires")
        );
    }

    #[test]
    fn ambiguous_xml_requires_explicit_override() {
        let error = InputDocument::new(
            "paper.xml",
            "application/xml",
            Bytes::from_static(b"<article />"),
        )
        .kind()
        .unwrap_err();

        assert!(error.to_string().contains("explicit input_format override"));
    }

    #[test]
    fn remote_convert_options_default_omits_picture_description_preset() {
        let options = RemoteConvertOptions::default();
        assert!(
            options.picture_description_preset.is_none(),
            "default RemoteConvertOptions must not carry a picture_description_preset so the legacy custom VLM bundle is preserved"
        );
    }

    #[test]
    fn remote_convert_options_round_trip_picture_description_preset() {
        let options = RemoteConvertOptions {
            chunker: ChunkerKind::None,
            chunking: ChunkingOptions::default(),
            pipeline: None,
            picture_description_preset: Some("granite_vision".to_string()),
        };
        assert_eq!(
            options.picture_description_preset.as_deref(),
            Some("granite_vision")
        );
    }
}
