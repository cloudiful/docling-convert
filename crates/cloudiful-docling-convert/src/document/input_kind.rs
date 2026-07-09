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

#[derive(Debug, Clone, Copy)]
struct InputKindSpec {
    kind: InputKind,
    extensions: &'static [&'static str],
    media_types: &'static [&'static str],
    default_extension: &'static str,
    default_media_type: &'static str,
    from_formats_value: &'static str,
    parse_aliases: &'static [&'static str],
    reading_label: &'static str,
    auto_detect: bool,
    generic_convert_options: bool,
    supports_vlm: bool,
}

const INPUT_KIND_SPECS: &[InputKindSpec] = &[
    InputKindSpec {
        kind: InputKind::Pdf,
        extensions: &["pdf"],
        media_types: &["application/pdf"],
        default_extension: "pdf",
        default_media_type: "application/pdf",
        from_formats_value: "pdf",
        parse_aliases: &["pdf"],
        reading_label: "Reading PDF...",
        auto_detect: true,
        generic_convert_options: false,
        supports_vlm: true,
    },
    InputKindSpec {
        kind: InputKind::Docx,
        extensions: &["docx"],
        media_types: &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"],
        default_extension: "docx",
        default_media_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        from_formats_value: "docx",
        parse_aliases: &["docx"],
        reading_label: "Reading DOCX...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: true,
    },
    InputKindSpec {
        kind: InputKind::Pptx,
        extensions: &["pptx"],
        media_types: &["application/vnd.openxmlformats-officedocument.presentationml.presentation"],
        default_extension: "pptx",
        default_media_type: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        from_formats_value: "pptx",
        parse_aliases: &["pptx"],
        reading_label: "Reading PPTX...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Html,
        extensions: &["html", "htm", "xhtml"],
        media_types: &["text/html", "application/xhtml+xml"],
        default_extension: "html",
        default_media_type: "text/html",
        from_formats_value: "html",
        parse_aliases: &["html"],
        reading_label: "Reading HTML...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Asciidoc,
        extensions: &["adoc", "asciidoc", "asc"],
        media_types: &["text/asciidoc", "text/x-asciidoc"],
        default_extension: "adoc",
        default_media_type: "text/asciidoc",
        from_formats_value: "asciidoc",
        parse_aliases: &["asciidoc", "adoc"],
        reading_label: "Reading AsciiDoc...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Markdown,
        extensions: &["md", "markdown"],
        media_types: &["text/markdown", "text/x-markdown"],
        default_extension: "md",
        default_media_type: "text/markdown",
        from_formats_value: "md",
        parse_aliases: &["markdown", "md"],
        reading_label: "Reading Markdown...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: true,
    },
    InputKindSpec {
        kind: InputKind::Csv,
        extensions: &["csv"],
        media_types: &["text/csv", "application/csv"],
        default_extension: "csv",
        default_media_type: "text/csv",
        from_formats_value: "csv",
        parse_aliases: &["csv"],
        reading_label: "Reading CSV...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Xlsx,
        extensions: &["xlsx"],
        media_types: &["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"],
        default_extension: "xlsx",
        default_media_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        from_formats_value: "xlsx",
        parse_aliases: &["xlsx"],
        reading_label: "Reading XLSX...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Odt,
        extensions: &["odt"],
        media_types: &["application/vnd.oasis.opendocument.text"],
        default_extension: "odt",
        default_media_type: "application/vnd.oasis.opendocument.text",
        from_formats_value: "odt",
        parse_aliases: &["odt"],
        reading_label: "Reading ODT...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Ods,
        extensions: &["ods"],
        media_types: &["application/vnd.oasis.opendocument.spreadsheet"],
        default_extension: "ods",
        default_media_type: "application/vnd.oasis.opendocument.spreadsheet",
        from_formats_value: "ods",
        parse_aliases: &["ods"],
        reading_label: "Reading ODS...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Odp,
        extensions: &["odp"],
        media_types: &["application/vnd.oasis.opendocument.presentation"],
        default_extension: "odp",
        default_media_type: "application/vnd.oasis.opendocument.presentation",
        from_formats_value: "odp",
        parse_aliases: &["odp"],
        reading_label: "Reading ODP...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Epub,
        extensions: &["epub"],
        media_types: &["application/epub+zip"],
        default_extension: "epub",
        default_media_type: "application/epub+zip",
        from_formats_value: "epub",
        parse_aliases: &["epub"],
        reading_label: "Reading EPUB...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Email,
        extensions: &["eml", "msg"],
        media_types: &["message/rfc822", "application/vnd.ms-outlook"],
        default_extension: "eml",
        default_media_type: "message/rfc822",
        from_formats_value: "email",
        parse_aliases: &["email", "eml", "msg"],
        reading_label: "Reading email...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Image,
        extensions: &[
            "png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp", "svg",
        ],
        media_types: &[
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/bmp",
            "image/tiff",
            "image/webp",
            "image/svg+xml",
        ],
        default_extension: "png",
        default_media_type: "image/png",
        from_formats_value: "image",
        parse_aliases: &["image"],
        reading_label: "Reading image...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: true,
    },
    InputKindSpec {
        kind: InputKind::XmlUspto,
        extensions: &["xml"],
        media_types: &["application/xml", "text/xml"],
        default_extension: "xml",
        default_media_type: "application/xml",
        from_formats_value: "xml_uspto",
        parse_aliases: &["xml_uspto"],
        reading_label: "Reading XML USPTO...",
        auto_detect: false,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::XmlJats,
        extensions: &["xml"],
        media_types: &["application/xml", "text/xml"],
        default_extension: "xml",
        default_media_type: "application/xml",
        from_formats_value: "xml_jats",
        parse_aliases: &["xml_jats"],
        reading_label: "Reading XML JATS...",
        auto_detect: false,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::XmlXbrl,
        extensions: &["xml"],
        media_types: &["application/xml", "text/xml"],
        default_extension: "xml",
        default_media_type: "application/xml",
        from_formats_value: "xml_xbrl",
        parse_aliases: &["xml_xbrl"],
        reading_label: "Reading XML XBRL...",
        auto_detect: false,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::XmlDoclang,
        extensions: &["xml"],
        media_types: &["application/xml", "text/xml"],
        default_extension: "xml",
        default_media_type: "application/xml",
        from_formats_value: "xml_doclang",
        parse_aliases: &["xml_doclang"],
        reading_label: "Reading XML Docling...",
        auto_detect: false,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::MetsGbs,
        extensions: &["xml"],
        media_types: &["application/xml", "text/xml"],
        default_extension: "xml",
        default_media_type: "application/xml",
        from_formats_value: "mets_gbs",
        parse_aliases: &["mets_gbs"],
        reading_label: "Reading METS GBS...",
        auto_detect: false,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::JsonDocling,
        extensions: &["json"],
        media_types: &["application/json", "text/json"],
        default_extension: "json",
        default_media_type: "application/json",
        from_formats_value: "json_docling",
        parse_aliases: &["json_docling"],
        reading_label: "Reading JSON Docling...",
        auto_detect: false,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Latex,
        extensions: &["tex"],
        media_types: &[
            "application/x-tex",
            "application/x-latex",
            "text/x-tex",
            "text/x-latex",
        ],
        default_extension: "tex",
        default_media_type: "application/x-tex",
        from_formats_value: "latex",
        parse_aliases: &["latex", "tex"],
        reading_label: "Reading LaTeX...",
        auto_detect: true,
        generic_convert_options: true,
        supports_vlm: false,
    },
    InputKindSpec {
        kind: InputKind::Text,
        extensions: &["txt"],
        media_types: &["text/plain"],
        default_extension: "txt",
        default_media_type: "text/plain",
        from_formats_value: "text",
        parse_aliases: &["text", "txt"],
        reading_label: "Reading text file...",
        auto_detect: true,
        generic_convert_options: false,
        supports_vlm: false,
    },
];

impl InputKind {
    pub fn from_path(path: &Path) -> Option<Self> {
        let file_name = path.file_name().and_then(|name| name.to_str())?;
        Self::from_filename_and_media_type(file_name, None)
    }

    pub fn from_filename_and_media_type(filename: &str, media_type: Option<&str>) -> Option<Self> {
        let ext = normalized_extension(filename);
        let media_type = normalized_media_type(media_type);

        if Self::requires_explicit_override(filename, media_type.as_deref()) {
            return None;
        }

        detect_auto_kind(ext.as_deref(), media_type.as_deref())
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
        self.spec().default_media_type
    }

    pub fn canonical_media_type(self, filename: &str, media_type: Option<&str>) -> &'static str {
        let ext = normalized_extension(filename);
        let media_type = normalized_media_type(media_type);

        match self {
            Self::Html => match (ext.as_deref(), media_type.as_deref()) {
                (Some("xhtml"), _) | (_, Some("application/xhtml+xml")) => "application/xhtml+xml",
                _ => self.media_type(),
            },
            Self::Email => match (ext.as_deref(), media_type.as_deref()) {
                (Some("msg"), _) | (_, Some("application/vnd.ms-outlook")) => {
                    "application/vnd.ms-outlook"
                }
                _ => self.media_type(),
            },
            Self::Image => image_media_type(ext.as_deref(), media_type.as_deref()),
            _ => self.media_type(),
        }
    }

    pub fn default_extension(self) -> &'static str {
        self.spec().default_extension
    }

    pub fn default_extension_for_media_type(self, media_type: Option<&str>) -> &'static str {
        let media_type = normalized_media_type(media_type);

        match self {
            Self::Html => match media_type.as_deref() {
                Some("application/xhtml+xml") => "xhtml",
                _ => self.default_extension(),
            },
            Self::Email => match media_type.as_deref() {
                Some("application/vnd.ms-outlook") => "msg",
                _ => self.default_extension(),
            },
            Self::Image => image_extension(media_type.as_deref()),
            _ => self.default_extension(),
        }
    }

    pub fn reading_label(self) -> &'static str {
        self.spec().reading_label
    }

    pub fn from_formats_value(self) -> &'static str {
        self.spec().from_formats_value
    }

    pub fn uses_generic_convert_options(self) -> bool {
        self.spec().generic_convert_options
    }

    pub fn supports_vlm(self) -> bool {
        self.spec().supports_vlm
    }

    fn spec(self) -> &'static InputKindSpec {
        INPUT_KIND_SPECS
            .iter()
            .find(|spec| spec.kind == self)
            .expect("missing InputKindSpec")
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
        let normalized = value.trim().to_ascii_lowercase();

        INPUT_KIND_SPECS
            .iter()
            .find(|spec| spec.parse_aliases.contains(&normalized.as_str()))
            .map(|spec| spec.kind)
            .ok_or_else(|| {
                PdfConvertError::validation_error(
                    "input_format",
                    format!("unsupported input format: {}", normalized),
                )
            })
    }
}

fn detect_auto_kind(extension: Option<&str>, media_type: Option<&str>) -> Option<InputKind> {
    INPUT_KIND_SPECS
        .iter()
        .filter(|spec| spec.auto_detect)
        .find(|spec| {
            extension.is_some_and(|ext| spec.extensions.contains(&ext))
                || media_type.is_some_and(|mime| spec.media_types.contains(&mime))
        })
        .map(|spec| spec.kind)
}

fn image_media_type(extension: Option<&str>, media_type: Option<&str>) -> &'static str {
    match (extension, media_type) {
        (Some("jpg" | "jpeg"), _) | (_, Some("image/jpeg")) => "image/jpeg",
        (Some("gif"), _) | (_, Some("image/gif")) => "image/gif",
        (Some("bmp"), _) | (_, Some("image/bmp")) => "image/bmp",
        (Some("tif" | "tiff"), _) | (_, Some("image/tiff")) => "image/tiff",
        (Some("webp"), _) | (_, Some("image/webp")) => "image/webp",
        (Some("svg"), _) | (_, Some("image/svg+xml")) => "image/svg+xml",
        _ => "image/png",
    }
}

fn image_extension(media_type: Option<&str>) -> &'static str {
    match media_type {
        Some("image/jpeg") => "jpg",
        Some("image/gif") => "gif",
        Some("image/bmp") => "bmp",
        Some("image/tiff") => "tiff",
        Some("image/webp") => "webp",
        Some("image/svg+xml") => "svg",
        _ => "png",
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
    fn metadata_methods_match_specs() {
        for spec in INPUT_KIND_SPECS {
            assert_eq!(spec.kind.default_extension(), spec.default_extension);
            assert_eq!(spec.kind.media_type(), spec.default_media_type);
            assert_eq!(spec.kind.from_formats_value(), spec.from_formats_value);
            assert_eq!(spec.kind.reading_label(), spec.reading_label);
            assert_eq!(
                spec.kind.uses_generic_convert_options(),
                spec.generic_convert_options
            );
            assert_eq!(spec.kind.supports_vlm(), spec.supports_vlm);
        }
    }

    #[test]
    fn detects_first_wave_and_latex_input_kinds_from_extension() {
        for spec in INPUT_KIND_SPECS.iter().filter(|spec| spec.auto_detect) {
            for extension in spec.extensions {
                let filename = format!("sample.{}", extension);
                assert_eq!(
                    InputKind::from_path(Path::new(&filename)),
                    Some(spec.kind),
                    "failed to detect extension '{}'",
                    extension
                );
            }
        }
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
        for spec in INPUT_KIND_SPECS.iter().filter(|spec| spec.auto_detect) {
            for media_type in spec.media_types {
                assert_eq!(
                    InputKind::from_filename_and_media_type("upload", Some(media_type)),
                    Some(spec.kind),
                    "failed to detect media type '{}'",
                    media_type
                );
            }
        }
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
        assert_eq!("msg".parse::<InputKind>().unwrap(), InputKind::Email);
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
