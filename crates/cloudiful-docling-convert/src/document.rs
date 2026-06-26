use std::path::Path;
use std::str::FromStr;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{PdfConvertError, Result};
use crate::models::{Bookmark, ChunkMetadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Json,
    Md,
    Text,
}

impl OutputFormat {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Md => "md",
            Self::Text => "text",
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
            other => Err(PdfConvertError::validation_error(
                "format",
                format!("unsupported output format: {}", other),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Pdf,
    Docx,
    Markdown,
    Text,
}

impl InputKind {
    pub fn from_path(path: &Path) -> Option<Self> {
        let file_name = path.file_name().and_then(|name| name.to_str())?;
        Self::from_filename_and_media_type(file_name, None)
    }

    pub fn from_filename_and_media_type(filename: &str, media_type: Option<&str>) -> Option<Self> {
        let ext = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        let media_type = media_type.map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or(value)
                .trim()
                .to_ascii_lowercase()
        });

        match (ext.as_deref(), media_type.as_deref()) {
            (Some("pdf"), _) | (_, Some("application/pdf")) => Some(Self::Pdf),
            (Some("docx"), _)
            | (
                _,
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            ) => Some(Self::Docx),
            (Some("md"), _)
            | (Some("markdown"), _)
            | (_, Some("text/markdown"))
            | (_, Some("text/x-markdown")) => Some(Self::Markdown),
            (Some("txt"), _) | (_, Some("text/plain")) => Some(Self::Text),
            _ => None,
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Markdown => "text/markdown",
            Self::Text => "text/plain",
        }
    }

    pub fn default_extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Markdown => "md",
            Self::Text => "txt",
        }
    }

    pub fn reading_label(self) -> &'static str {
        match self {
            Self::Pdf => "Reading PDF...",
            Self::Docx => "Reading DOCX...",
            Self::Markdown => "Reading Markdown...",
            Self::Text => "Reading text file...",
        }
    }

    pub fn from_formats_value(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Markdown => "md",
            Self::Text => "text",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputDocument {
    pub filename: String,
    pub media_type: String,
    pub bytes: Bytes,
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
        }
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

        Ok(Self::new(filename, kind.media_type(), bytes))
    }

    pub fn kind(&self) -> Result<InputKind> {
        InputKind::from_filename_and_media_type(&self.filename, Some(&self.media_type)).ok_or_else(
            || {
                PdfConvertError::validation_error(
                    "input",
                    format!(
                        "unsupported input type for '{}' ({})",
                        self.filename, self.media_type
                    ),
                )
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
        match (&kind, &self.options) {
            (InputKind::Pdf, ConvertOptions::Pdf(_))
            | (InputKind::Docx, ConvertOptions::Generic(_))
            | (InputKind::Markdown, ConvertOptions::Generic(_))
            | (InputKind::Text, ConvertOptions::Text(_)) => {}
            (InputKind::Pdf, _) => {
                return Err(PdfConvertError::validation_error(
                    "options",
                    "PDF input requires Pdf convert options",
                ));
            }
            (InputKind::Docx | InputKind::Markdown, _) => {
                return Err(PdfConvertError::validation_error(
                    "options",
                    "docx/md input requires GenericFileConvertOptions",
                ));
            }
            (InputKind::Text, _) => {
                return Err(PdfConvertError::validation_error(
                    "options",
                    "txt input requires TextConvertOptions",
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
    fn detects_supported_input_kinds() {
        assert_eq!(
            InputKind::from_path(Path::new("a.pdf")),
            Some(InputKind::Pdf)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.docx")),
            Some(InputKind::Docx)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.md")),
            Some(InputKind::Markdown)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.txt")),
            Some(InputKind::Text)
        );
        assert_eq!(InputKind::from_path(Path::new("a.csv")), None);
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
}
