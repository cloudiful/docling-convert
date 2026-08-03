use serde_json::json;

use super::DocumentConverter;
use crate::document::{
    ConvertedDocument, ConvertedDocumentMetadata, InputDocument, InputKind, OutputFormat,
    TextConvertOptions,
};
use crate::error::Result;

impl DocumentConverter {
    pub(super) fn convert_text(
        &self,
        input: &InputDocument,
        options: &TextConvertOptions,
        output_formats: &[OutputFormat],
    ) -> Result<ConvertedDocument> {
        let mut content = String::from_utf8_lossy(input.bytes.as_ref()).into_owned();
        if options.trim_utf8_bom {
            content = content.trim_start_matches('\u{feff}').to_string();
        }
        if options.normalize_line_endings {
            content = content.replace("\r\n", "\n").replace('\r', "\n");
        }

        let html = output_formats
            .iter()
            .any(|format| matches!(format, OutputFormat::Html))
            .then(|| {
                format!(
                    "<!DOCTYPE html><html><body><pre>{}</pre></body></html>",
                    escape_html(&content)
                )
            });
        let doctags = output_formats
            .iter()
            .any(|format| matches!(format, OutputFormat::Doctags))
            .then(|| content.clone());
        let doclang = output_formats
            .iter()
            .any(|format| matches!(format, OutputFormat::Doclang | OutputFormat::Vtt))
            .then(|| content.clone());
        let markdown = output_formats
            .iter()
            .any(|format| matches!(format, OutputFormat::Md))
            .then(|| content.clone());
        let text = output_formats
            .iter()
            .any(|format| matches!(format, OutputFormat::Text))
            .then(|| content.clone());
        let json = output_formats
            .iter()
            .any(|format| matches!(format, OutputFormat::Json))
            .then(|| {
                json!({
                    "document": {
                        "md_content": content.clone(),
                        "text_content": content.clone(),
                        "html_content": html.clone(),
                        "doctags_content": doctags.clone(),
                        "doclang_content": doclang.clone(),
                    }
                })
            });

        Ok(ConvertedDocument {
            filename: input.filename.clone(),
            markdown,
            text,
            json,
            html,
            doctags,
            doclang,
            chunks: Vec::new(),
            chunk_response: None,
            archive: None,
            metadata: ConvertedDocumentMetadata {
                input_kind: InputKind::Text,
                media_type: input.media_type.clone(),
            },
            errors: Vec::new(),
        })
    }
}

fn escape_html(content: &str) -> String {
    let mut escaped = String::with_capacity(content.len());
    for ch in content.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
