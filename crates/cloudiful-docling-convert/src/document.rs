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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Pdf,
    Docx,
    Pptx,
    Html,
    Asciidoc,
    Markdown,
    Csv,
    Xlsx,
    Odt,
    Ods,
    Odp,
    Epub,
    Email,
    Image,
    Text,
}

impl InputKind {
    pub fn from_path(path: &Path) -> Option<Self> {
        let file_name = path.file_name().and_then(|name| name.to_str())?;
        Self::from_filename_and_media_type(file_name, None)
    }

    pub fn from_filename_and_media_type(filename: &str, media_type: Option<&str>) -> Option<Self> {
        let ext = normalized_extension(filename);
        let media_type = normalized_media_type(media_type);

        match (ext.as_deref(), media_type.as_deref()) {
            (Some("pdf"), _) | (_, Some("application/pdf")) => Some(Self::Pdf),
            (Some("docx"), _)
            | (
                _,
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            ) => Some(Self::Docx),
            (Some("pptx"), _)
            | (
                _,
                Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
            ) => Some(Self::Pptx),
            (Some("html"), _)
            | (Some("htm"), _)
            | (Some("xhtml"), _)
            | (_, Some("text/html"))
            | (_, Some("application/xhtml+xml")) => Some(Self::Html),
            (Some("adoc"), _)
            | (Some("asciidoc"), _)
            | (Some("asc"), _)
            | (_, Some("text/asciidoc"))
            | (_, Some("text/x-asciidoc")) => Some(Self::Asciidoc),
            (Some("md"), _)
            | (Some("markdown"), _)
            | (_, Some("text/markdown"))
            | (_, Some("text/x-markdown")) => Some(Self::Markdown),
            (Some("csv"), _) | (_, Some("text/csv")) | (_, Some("application/csv")) => {
                Some(Self::Csv)
            }
            (Some("xlsx"), _)
            | (_, Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")) => {
                Some(Self::Xlsx)
            }
            (Some("odt"), _) | (_, Some("application/vnd.oasis.opendocument.text")) => {
                Some(Self::Odt)
            }
            (Some("ods"), _) | (_, Some("application/vnd.oasis.opendocument.spreadsheet")) => {
                Some(Self::Ods)
            }
            (Some("odp"), _) | (_, Some("application/vnd.oasis.opendocument.presentation")) => {
                Some(Self::Odp)
            }
            (Some("epub"), _) | (_, Some("application/epub+zip")) => Some(Self::Epub),
            (Some("eml"), _)
            | (Some("msg"), _)
            | (_, Some("message/rfc822"))
            | (_, Some("application/vnd.ms-outlook")) => Some(Self::Email),
            (Some("png"), _)
            | (Some("jpg"), _)
            | (Some("jpeg"), _)
            | (Some("gif"), _)
            | (Some("bmp"), _)
            | (Some("tif"), _)
            | (Some("tiff"), _)
            | (Some("webp"), _)
            | (Some("svg"), _)
            | (_, Some("image/png"))
            | (_, Some("image/jpeg"))
            | (_, Some("image/gif"))
            | (_, Some("image/bmp"))
            | (_, Some("image/tiff"))
            | (_, Some("image/webp"))
            | (_, Some("image/svg+xml")) => Some(Self::Image),
            (Some("txt"), _) | (_, Some("text/plain")) => Some(Self::Text),
            _ => None,
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
            Self::Html => "text/html",
            Self::Asciidoc => "text/asciidoc",
            Self::Markdown => "text/markdown",
            Self::Csv => "text/csv",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Odt => "application/vnd.oasis.opendocument.text",
            Self::Ods => "application/vnd.oasis.opendocument.spreadsheet",
            Self::Odp => "application/vnd.oasis.opendocument.presentation",
            Self::Epub => "application/epub+zip",
            Self::Email => "message/rfc822",
            Self::Image => "image/png",
            Self::Text => "text/plain",
        }
    }

    pub fn canonical_media_type(self, filename: &str, media_type: Option<&str>) -> &'static str {
        let ext = normalized_extension(filename);
        let media_type = normalized_media_type(media_type);

        match self {
            Self::Html => match (ext.as_deref(), media_type.as_deref()) {
                (Some("xhtml"), _) | (_, Some("application/xhtml+xml")) => "application/xhtml+xml",
                _ => "text/html",
            },
            Self::Asciidoc => "text/asciidoc",
            Self::Email => match (ext.as_deref(), media_type.as_deref()) {
                (Some("msg"), _) | (_, Some("application/vnd.ms-outlook")) => {
                    "application/vnd.ms-outlook"
                }
                _ => "message/rfc822",
            },
            Self::Image => match (ext.as_deref(), media_type.as_deref()) {
                (Some("jpg"), _) | (Some("jpeg"), _) | (_, Some("image/jpeg")) => "image/jpeg",
                (Some("gif"), _) | (_, Some("image/gif")) => "image/gif",
                (Some("bmp"), _) | (_, Some("image/bmp")) => "image/bmp",
                (Some("tif"), _) | (Some("tiff"), _) | (_, Some("image/tiff")) => "image/tiff",
                (Some("webp"), _) | (_, Some("image/webp")) => "image/webp",
                (Some("svg"), _) | (_, Some("image/svg+xml")) => "image/svg+xml",
                _ => "image/png",
            },
            _ => self.media_type(),
        }
    }

    pub fn default_extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Pptx => "pptx",
            Self::Html => "html",
            Self::Asciidoc => "adoc",
            Self::Markdown => "md",
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
            Self::Odt => "odt",
            Self::Ods => "ods",
            Self::Odp => "odp",
            Self::Epub => "epub",
            Self::Email => "eml",
            Self::Image => "png",
            Self::Text => "txt",
        }
    }

    pub fn default_extension_for_media_type(self, media_type: Option<&str>) -> &'static str {
        let media_type = normalized_media_type(media_type);

        match self {
            Self::Html => match media_type.as_deref() {
                Some("application/xhtml+xml") => "xhtml",
                _ => "html",
            },
            Self::Email => match media_type.as_deref() {
                Some("application/vnd.ms-outlook") => "msg",
                _ => "eml",
            },
            Self::Image => match media_type.as_deref() {
                Some("image/jpeg") => "jpg",
                Some("image/gif") => "gif",
                Some("image/bmp") => "bmp",
                Some("image/tiff") => "tiff",
                Some("image/webp") => "webp",
                Some("image/svg+xml") => "svg",
                _ => "png",
            },
            _ => self.default_extension(),
        }
    }

    pub fn reading_label(self) -> &'static str {
        match self {
            Self::Pdf => "Reading PDF...",
            Self::Docx => "Reading DOCX...",
            Self::Pptx => "Reading PPTX...",
            Self::Html => "Reading HTML...",
            Self::Asciidoc => "Reading AsciiDoc...",
            Self::Markdown => "Reading Markdown...",
            Self::Csv => "Reading CSV...",
            Self::Xlsx => "Reading XLSX...",
            Self::Odt => "Reading ODT...",
            Self::Ods => "Reading ODS...",
            Self::Odp => "Reading ODP...",
            Self::Epub => "Reading EPUB...",
            Self::Email => "Reading email...",
            Self::Image => "Reading image...",
            Self::Text => "Reading text file...",
        }
    }

    pub fn from_formats_value(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Pptx => "pptx",
            Self::Html => "html",
            Self::Asciidoc => "asciidoc",
            Self::Markdown => "md",
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
            Self::Odt => "odt",
            Self::Ods => "ods",
            Self::Odp => "odp",
            Self::Epub => "epub",
            Self::Email => "email",
            Self::Image => "image",
            Self::Text => "text",
        }
    }
}

fn normalized_extension(filename: &str) -> Option<String> {
    Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn normalized_media_type(media_type: Option<&str>) -> Option<String> {
    media_type.map(|value| {
        value
            .split(';')
            .next()
            .unwrap_or(value)
            .trim()
            .to_ascii_lowercase()
    })
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

        Ok(Self::new(
            filename,
            kind.canonical_media_type(filename, None),
            bytes,
        ))
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
            | (
                InputKind::Docx
                | InputKind::Pptx
                | InputKind::Html
                | InputKind::Asciidoc
                | InputKind::Markdown
                | InputKind::Csv
                | InputKind::Xlsx
                | InputKind::Odt
                | InputKind::Ods
                | InputKind::Odp
                | InputKind::Epub
                | InputKind::Email
                | InputKind::Image,
                ConvertOptions::Generic(_),
            )
            | (InputKind::Text, ConvertOptions::Text(_)) => {}
            (InputKind::Pdf, _) => {
                return Err(PdfConvertError::validation_error(
                    "options",
                    "PDF input requires Pdf convert options",
                ));
            }
            (
                InputKind::Docx
                | InputKind::Pptx
                | InputKind::Html
                | InputKind::Asciidoc
                | InputKind::Markdown
                | InputKind::Csv
                | InputKind::Xlsx
                | InputKind::Odt
                | InputKind::Ods
                | InputKind::Odp
                | InputKind::Epub
                | InputKind::Email
                | InputKind::Image,
                _,
            ) => {
                return Err(PdfConvertError::validation_error(
                    "options",
                    "non-pdf, non-text input requires GenericFileConvertOptions",
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
            InputKind::from_path(Path::new("a.pptx")),
            Some(InputKind::Pptx)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.html")),
            Some(InputKind::Html)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.adoc")),
            Some(InputKind::Asciidoc)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.md")),
            Some(InputKind::Markdown)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.csv")),
            Some(InputKind::Csv)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.xlsx")),
            Some(InputKind::Xlsx)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.odt")),
            Some(InputKind::Odt)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.ods")),
            Some(InputKind::Ods)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.odp")),
            Some(InputKind::Odp)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.epub")),
            Some(InputKind::Epub)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.eml")),
            Some(InputKind::Email)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.jpg")),
            Some(InputKind::Image)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.txt")),
            Some(InputKind::Text)
        );
        assert_eq!(InputKind::from_path(Path::new("a.exe")), None);
    }

    #[test]
    fn detects_supported_input_kinds_from_mime_type() {
        assert_eq!(
            InputKind::from_filename_and_media_type("downloaded", Some("text/csv")),
            Some(InputKind::Csv)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("downloaded", Some("application/epub+zip")),
            Some(InputKind::Epub)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("downloaded", Some("image/webp")),
            Some(InputKind::Image)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("downloaded", Some("message/rfc822")),
            Some(InputKind::Email)
        );
    }

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
    fn prefers_extension_specific_media_types() {
        assert_eq!(
            InputKind::Image.canonical_media_type("cover.jpg", None),
            "image/jpeg"
        );
        assert_eq!(
            InputKind::Email.canonical_media_type("message.msg", None),
            "application/vnd.ms-outlook"
        );
        assert_eq!(
            InputKind::Image.default_extension_for_media_type(Some("image/webp")),
            "webp"
        );
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
