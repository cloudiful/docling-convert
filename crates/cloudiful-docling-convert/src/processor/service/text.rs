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
                        "md_content": content,
                        "text_content": content,
                    }
                })
            });

        let raw_result = json!({
            "document": {
                "md_content": markdown.clone(),
                "text_content": text.clone(),
                "json_content": json.clone(),
            }
        });

        Ok(ConvertedDocument {
            filename: input.filename.clone(),
            markdown,
            text,
            json,
            chunks: vec![crate::document::ConvertedChunk {
                metadata: None,
                markdown: Some(content.clone()),
                text: Some(content),
                json: raw_result
                    .get("document")
                    .and_then(|document| document.get("json_content"))
                    .cloned(),
                raw_result,
            }],
            metadata: ConvertedDocumentMetadata {
                input_kind: InputKind::Text,
                media_type: input.media_type.clone(),
                page_count: None,
                outlines: Vec::new(),
            },
            errors: Vec::new(),
        })
    }
}
