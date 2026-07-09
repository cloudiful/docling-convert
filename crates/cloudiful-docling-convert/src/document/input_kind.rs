use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{PdfConvertError, Result};

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
    XmlUspto,
    XmlJats,
    XmlXbrl,
    XmlDoclang,
    MetsGbs,
    JsonDocling,
    Latex,
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
            (Some("tex"), _)
            | (_, Some("application/x-tex"))
            | (_, Some("application/x-latex"))
            | (_, Some("text/x-tex"))
            | (_, Some("text/x-latex")) => Some(Self::Latex),
            (Some("txt"), _) | (_, Some("text/plain")) => Some(Self::Text),
            _ => None,
        }
    }

    pub fn requires_explicit_override(filename: &str, media_type: Option<&str>) -> bool {
        let ext = normalized_extension(filename);
        let media_type = normalized_media_type(media_type);

        matches!(ext.as_deref(), Some("xml") | Some("json"))
            || matches!(
                media_type.as_deref(),
                Some("application/xml")
                    | Some("text/xml")
                    | Some("application/json")
                    | Some("text/json")
            )
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
            Self::XmlUspto | Self::XmlJats | Self::XmlXbrl | Self::XmlDoclang | Self::MetsGbs => {
                "application/xml"
            }
            Self::JsonDocling => "application/json",
            Self::Latex => "application/x-tex",
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
            Self::XmlUspto | Self::XmlJats | Self::XmlXbrl | Self::XmlDoclang | Self::MetsGbs => {
                "xml"
            }
            Self::JsonDocling => "json",
            Self::Latex => "tex",
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
            Self::XmlUspto => "Reading XML USPTO...",
            Self::XmlJats => "Reading XML JATS...",
            Self::XmlXbrl => "Reading XML XBRL...",
            Self::XmlDoclang => "Reading XML Docling...",
            Self::MetsGbs => "Reading METS GBS...",
            Self::JsonDocling => "Reading JSON Docling...",
            Self::Latex => "Reading LaTeX...",
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
            Self::XmlUspto => "xml_uspto",
            Self::XmlJats => "xml_jats",
            Self::XmlXbrl => "xml_xbrl",
            Self::XmlDoclang => "xml_doclang",
            Self::MetsGbs => "mets_gbs",
            Self::JsonDocling => "json_docling",
            Self::Latex => "latex",
            Self::Text => "text",
        }
    }

    pub fn uses_generic_convert_options(self) -> bool {
        !matches!(self, Self::Pdf | Self::Text)
    }

    pub fn supports_vlm(self) -> bool {
        matches!(self, Self::Pdf | Self::Docx | Self::Markdown | Self::Image)
    }
}

impl std::fmt::Display for InputKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.from_formats_value())
    }
}

impl FromStr for InputKind {
    type Err = PdfConvertError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pdf" => Ok(Self::Pdf),
            "docx" => Ok(Self::Docx),
            "pptx" => Ok(Self::Pptx),
            "html" => Ok(Self::Html),
            "asciidoc" | "adoc" => Ok(Self::Asciidoc),
            "markdown" | "md" => Ok(Self::Markdown),
            "csv" => Ok(Self::Csv),
            "xlsx" => Ok(Self::Xlsx),
            "odt" => Ok(Self::Odt),
            "ods" => Ok(Self::Ods),
            "odp" => Ok(Self::Odp),
            "epub" => Ok(Self::Epub),
            "email" | "eml" | "msg" => Ok(Self::Email),
            "image" => Ok(Self::Image),
            "xml_uspto" => Ok(Self::XmlUspto),
            "xml_jats" => Ok(Self::XmlJats),
            "xml_xbrl" => Ok(Self::XmlXbrl),
            "xml_doclang" => Ok(Self::XmlDoclang),
            "mets_gbs" => Ok(Self::MetsGbs),
            "json_docling" => Ok(Self::JsonDocling),
            "latex" | "tex" => Ok(Self::Latex),
            "text" | "txt" => Ok(Self::Text),
            other => Err(PdfConvertError::validation_error(
                "input_format",
                format!("unsupported input format: {}", other),
            )),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_first_wave_and_latex_input_kinds() {
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
            InputKind::from_path(Path::new("a.png")),
            Some(InputKind::Image)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.tex")),
            Some(InputKind::Latex)
        );
        assert_eq!(
            InputKind::from_path(Path::new("a.txt")),
            Some(InputKind::Text)
        );
    }

    #[test]
    fn generic_xml_and_json_do_not_auto_map() {
        assert_eq!(InputKind::from_path(Path::new("a.xml")), None);
        assert_eq!(InputKind::from_path(Path::new("a.json")), None);
        assert_eq!(
            InputKind::from_filename_and_media_type("downloaded", Some("application/xml")),
            None
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("downloaded", Some("application/json")),
            None
        );
    }

    #[test]
    fn detects_supported_input_kinds_from_mime_type() {
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("application/pdf")),
            Some(InputKind::Pdf)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type(
                "upload",
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
            ),
            Some(InputKind::Docx)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type(
                "upload",
                Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
            ),
            Some(InputKind::Pptx)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("application/xhtml+xml")),
            Some(InputKind::Html)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("text/asciidoc")),
            Some(InputKind::Asciidoc)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("text/markdown")),
            Some(InputKind::Markdown)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("text/csv")),
            Some(InputKind::Csv)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type(
                "upload",
                Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
            ),
            Some(InputKind::Xlsx)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type(
                "upload",
                Some("application/vnd.oasis.opendocument.text")
            ),
            Some(InputKind::Odt)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type(
                "upload",
                Some("application/vnd.oasis.opendocument.spreadsheet")
            ),
            Some(InputKind::Ods)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type(
                "upload",
                Some("application/vnd.oasis.opendocument.presentation")
            ),
            Some(InputKind::Odp)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("application/epub+zip")),
            Some(InputKind::Epub)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("message/rfc822")),
            Some(InputKind::Email)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("image/webp")),
            Some(InputKind::Image)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("application/x-tex")),
            Some(InputKind::Latex)
        );
        assert_eq!(
            InputKind::from_filename_and_media_type("upload", Some("text/plain")),
            Some(InputKind::Text)
        );
    }

    #[test]
    fn reports_override_only_sources() {
        assert!(InputKind::requires_explicit_override("paper.xml", None));
        assert!(InputKind::requires_explicit_override(
            "paper",
            Some("application/xml")
        ));
        assert!(InputKind::requires_explicit_override("paper.json", None));
        assert!(InputKind::requires_explicit_override(
            "paper",
            Some("application/json")
        ));
        assert!(!InputKind::requires_explicit_override("paper.tex", None));
    }

    #[test]
    fn derives_second_wave_defaults() {
        assert_eq!(InputKind::XmlJats.default_extension(), "xml");
        assert_eq!(InputKind::JsonDocling.default_extension(), "json");
        assert_eq!(InputKind::Latex.default_extension(), "tex");
        assert_eq!(InputKind::XmlUspto.from_formats_value(), "xml_uspto");
        assert_eq!(InputKind::JsonDocling.from_formats_value(), "json_docling");
        assert_eq!(InputKind::Latex.from_formats_value(), "latex");
    }

    #[test]
    fn parses_input_format_strings() {
        assert_eq!("xml_jats".parse::<InputKind>().unwrap(), InputKind::XmlJats);
        assert_eq!(
            "json_docling".parse::<InputKind>().unwrap(),
            InputKind::JsonDocling
        );
        assert_eq!("latex".parse::<InputKind>().unwrap(), InputKind::Latex);
        assert!("xml".parse::<InputKind>().is_err());
    }

    #[test]
    fn preserves_specific_media_types() {
        assert_eq!(
            InputKind::Image.canonical_media_type("cover.jpg", None),
            "image/jpeg"
        );
        assert_eq!(
            InputKind::Email.canonical_media_type("message.msg", None),
            "application/vnd.ms-outlook"
        );
        assert_eq!(
            InputKind::Html.canonical_media_type("page.xhtml", None),
            "application/xhtml+xml"
        );
    }
}
