use std::path::Path;
use std::str::FromStr;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{PdfConvertError, Result};
use crate::models::{Bookmark, ChunkMetadata};

mod input_kind;

pub use input_kind::InputKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Json,
    Md,
    Text,
    Html,
    Doctags,
}

impl OutputFormat {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Md => "md",
            Self::Text => "text",
            Self::Html => "html",
            Self::Doctags => "doctags",
        }
    }

    pub fn extension(self) -> &'static str {
        self.as_api_value()
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
        match value {
            "json" => Ok(Self::Json),
            "md" => Ok(Self::Md),
            "text" => Ok(Self::Text),
            "html" => Ok(Self::Html),
            "doctags" => Ok(Self::Doctags),
            other => Err(PdfConvertError::validation_error(
                "format",
                format!("unsupported output format: {}", other),
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

        InputKind::from_filename_and_media_type(&self.filename, Some(&self.media_type))
            .ok_or_else(|| {
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
                PdfConvertError::validation_error(
                    "input",
                    reason,
                )
            })
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
                    "PDF input requires Pdf convert options",
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

        if let ConvertOptions::Pdf(options) = &self.options {
            options.validate()?;
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

#[derive(Debug, Clone)]
pub struct PdfConvertOptions {
    pub pages_per_file: u32,
    pub split_input: bool,
    pub split_by_bookmark: bool,
    pub chunking: bool,
    pub batch_size: usize,
}

impl Default for PdfConvertOptions {
    fn default() -> Self {
        Self {
            pages_per_file: 5,
            split_input: true,
            split_by_bookmark: false,
            chunking: false,
            batch_size: 2,
        }
    }
}

impl PdfConvertOptions {
    pub fn validate(&self) -> Result<()> {
        if self.pages_per_file == 0 {
            return Err(PdfConvertError::validation_error(
                "pages_per_file",
                "value must be 1 or greater",
            ));
        }

        if self.batch_size == 0 {
            return Err(PdfConvertError::validation_error(
                "batch_size",
                "value must be 1 or greater",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct GenericFileConvertOptions {
    pub chunking: bool,
}

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
pub struct ConvertedChunk {
    pub metadata: Option<ChunkMetadata>,
    pub markdown: Option<String>,
    pub text: Option<String>,
    pub json: Option<Value>,
    pub html: Option<String>,
    pub doctags: Option<String>,
    pub raw_result: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertedDocumentMetadata {
    pub input_kind: InputKind,
    pub media_type: String,
    pub page_count: Option<u32>,
    pub outlines: Vec<Bookmark>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertedDocument {
    pub filename: String,
    pub markdown: Option<String>,
    pub text: Option<String>,
    pub json: Option<Value>,
    pub html: Option<String>,
    pub doctags: Option<String>,
    pub chunks: Vec<ConvertedChunk>,
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
    fn output_format_supports_html_and_doctags() {
        assert_eq!("html".parse::<OutputFormat>().unwrap(), OutputFormat::Html);
        assert_eq!(
            "doctags".parse::<OutputFormat>().unwrap(),
            OutputFormat::Doctags
        );
        assert_eq!(OutputFormat::Html.extension(), "html");
        assert_eq!(OutputFormat::Doctags.extension(), "doctags");
    }

    #[test]
    fn request_validation_rejects_mismatched_options() {
        let request = ConvertRequest {
            input: InputDocument::new("a.txt", "text/plain", Bytes::from_static(b"hello")),
            output_formats: vec![OutputFormat::Text],
            options: ConvertOptions::Generic(GenericFileConvertOptions::default()),
        };

        let err = request.validate().unwrap_err();
        assert!(err.to_string().contains("TextConvertOptions"));
    }

    #[test]
    fn request_validation_rejects_zero_pages_per_file() {
        let request = ConvertRequest {
            input: InputDocument::new("a.pdf", "application/pdf", Bytes::from_static(b"%PDF")),
            output_formats: vec![OutputFormat::Text],
            options: ConvertOptions::Pdf(PdfConvertOptions {
                pages_per_file: 0,
                ..PdfConvertOptions::default()
            }),
        };

        let err = request.validate().unwrap_err();
        assert!(err.to_string().contains("pages_per_file"));
    }

    #[test]
    fn request_validation_rejects_zero_batch_size() {
        let request = ConvertRequest {
            input: InputDocument::new("a.pdf", "application/pdf", Bytes::from_static(b"%PDF")),
            output_formats: vec![OutputFormat::Text],
            options: ConvertOptions::Pdf(PdfConvertOptions {
                batch_size: 0,
                ..PdfConvertOptions::default()
            }),
        };

        let err = request.validate().unwrap_err();
        assert!(err.to_string().contains("batch_size"));
    }

    #[test]
    fn ambiguous_xml_requires_explicit_override() {
        let err = InputDocument::new(
            "paper.xml",
            "application/xml",
            Bytes::from_static(b"<article />"),
        )
        .kind()
        .unwrap_err();

        assert!(err.to_string().contains("explicit input_format override"));
    }

    #[test]
    fn override_resolves_ambiguous_sources() {
        let input = InputDocument::new(
            "paper.xml",
            "application/xml",
            Bytes::from_static(b"<article />"),
        )
        .with_input_kind(InputKind::XmlJats);

        assert_eq!(input.kind().unwrap(), InputKind::XmlJats);
    }

    #[test]
    fn from_path_and_bytes_with_kind_preserves_override_kind() {
        let input = InputDocument::from_path_and_bytes_with_kind(
            Path::new("filing.json"),
            Bytes::from_static(br#"{"schema":"docling"}"#),
            InputKind::JsonDocling,
        )
        .unwrap();

        assert_eq!(input.kind().unwrap(), InputKind::JsonDocling);
        assert_eq!(input.media_type, "application/json");
    }
}
