use std::collections::HashMap;

use axum::http::StatusCode;
use cloudiful_docling_convert::{ChunkerKind, ChunkingOptions, PipelineKind};
use serde::Deserialize;

use super::state::TaskConfig;
use super::support::{parse_input_format, parse_output_format};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TaskConfigInput {
    pub format: Option<String>,
    pub input_format: Option<String>,
    pub chunker: Option<String>,
    pub chunking: Option<bool>,
    pub pipeline: Option<String>,
    pub chunking_options: Option<ChunkingOptionsInput>,
    pub picture_description_preset: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChunkingOptionsInput {
    pub use_markdown_tables: Option<bool>,
    pub use_markdown_images: Option<bool>,
    pub image_placeholder: Option<String>,
    pub include_raw_text: Option<bool>,
    pub max_tokens: Option<u32>,
    pub tokenizer: Option<String>,
    pub merge_peers: Option<bool>,
}

impl TaskConfigInput {
    pub fn resolve(self) -> Result<TaskConfig, StatusCode> {
        let mut config = TaskConfig::default();

        if let Some(format) = self.format {
            parse_output_format(&format).map_err(|_| StatusCode::BAD_REQUEST)?;
            config.format = format;
        }
        if let Some(input_format) = self.input_format {
            parse_input_format(&input_format).map_err(|_| StatusCode::BAD_REQUEST)?;
            config.input_format = Some(input_format);
        }

        config.chunker = match self.chunker {
            Some(value) => value.parse().map_err(|_| StatusCode::BAD_REQUEST)?,
            None if self.chunking == Some(true) => ChunkerKind::Hybrid,
            None => ChunkerKind::None,
        };
        config.pipeline = self
            .pipeline
            .map(|value| value.parse::<PipelineKind>())
            .transpose()
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        config.picture_description_preset = self
            .picture_description_preset
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        if let Some(options) = self.chunking_options {
            apply_chunking_options(&mut config.chunking_options, options);
        }
        Ok(config)
    }

    pub fn from_multipart_fields(fields: &HashMap<String, String>) -> Result<Self, StatusCode> {
        let chunking_options = if fields.keys().any(|key| {
            matches!(
                key.as_str(),
                "use_markdown_tables"
                    | "use_markdown_images"
                    | "image_placeholder"
                    | "include_raw_text"
                    | "max_tokens"
                    | "tokenizer"
                    | "merge_peers"
            )
        }) {
            Some(ChunkingOptionsInput {
                use_markdown_tables: parse_optional_bool(fields, "use_markdown_tables")?,
                use_markdown_images: parse_optional_bool(fields, "use_markdown_images")?,
                image_placeholder: fields.get("image_placeholder").cloned(),
                include_raw_text: parse_optional_bool(fields, "include_raw_text")?,
                max_tokens: parse_optional(fields, "max_tokens")?,
                tokenizer: fields.get("tokenizer").cloned(),
                merge_peers: parse_optional_bool(fields, "merge_peers")?,
            })
        } else {
            None
        };

        Ok(Self {
            format: fields.get("format").cloned(),
            input_format: fields.get("input_format").cloned(),
            chunker: fields.get("chunker").cloned(),
            chunking: parse_optional_bool(fields, "chunking")?,
            pipeline: fields.get("pipeline").cloned(),
            chunking_options,
            picture_description_preset: fields.get("picture_description_preset").cloned(),
        })
    }
}

fn apply_chunking_options(options: &mut ChunkingOptions, input: ChunkingOptionsInput) {
    if let Some(value) = input.use_markdown_tables {
        options.use_markdown_tables = value;
    }
    if let Some(value) = input.use_markdown_images {
        options.use_markdown_images = value;
    }
    if let Some(value) = input.image_placeholder {
        options.image_placeholder = value;
    }
    if let Some(value) = input.include_raw_text {
        options.include_raw_text = value;
    }
    if input.max_tokens.is_some() {
        options.max_tokens = input.max_tokens;
    }
    if let Some(value) = input.tokenizer {
        options.tokenizer = Some(value);
    }
    if let Some(value) = input.merge_peers {
        options.merge_peers = value;
    }
}

fn parse_optional<T>(fields: &HashMap<String, String>, key: &str) -> Result<Option<T>, StatusCode>
where
    T: std::str::FromStr,
{
    match fields.get(key) {
        Some(value) => value
            .parse::<T>()
            .map(Some)
            .map_err(|_| StatusCode::BAD_REQUEST),
        None => Ok(None),
    }
}

fn parse_optional_bool(
    fields: &HashMap<String, String>,
    key: &str,
) -> Result<Option<bool>, StatusCode> {
    match fields.get(key) {
        Some(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(Some(true)),
            "false" | "0" => Ok(Some(false)),
            _ => Err(StatusCode::BAD_REQUEST),
        },
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_native_chunker_and_pipeline() {
        let config = TaskConfigInput {
            format: Some("chunks".into()),
            chunker: Some("hybrid".into()),
            pipeline: Some("asr".into()),
            ..TaskConfigInput::default()
        }
        .resolve()
        .unwrap();

        assert_eq!(config.chunker, ChunkerKind::Hybrid);
        assert_eq!(config.pipeline, Some(PipelineKind::Asr));
    }

    #[test]
    fn maps_legacy_chunking_true_to_hybrid() {
        let config = TaskConfigInput {
            chunking: Some(true),
            ..TaskConfigInput::default()
        }
        .resolve()
        .unwrap();
        assert_eq!(config.chunker, ChunkerKind::Hybrid);
    }

    #[test]
    fn multipart_parses_chunk_options_without_old_split_fields() {
        let fields = HashMap::from([
            ("format".to_string(), "chunks".to_string()),
            ("chunker".to_string(), "hierarchical".to_string()),
            ("include_raw_text".to_string(), "true".to_string()),
        ]);
        let config = TaskConfigInput::from_multipart_fields(&fields)
            .unwrap()
            .resolve()
            .unwrap();
        assert_eq!(config.chunker, ChunkerKind::Hierarchical);
        assert!(config.chunking_options.include_raw_text);
    }

    #[test]
    fn resolves_picture_description_preset_from_json_input() {
        let config = TaskConfigInput {
            picture_description_preset: Some("smolvlm".to_string()),
            ..TaskConfigInput::default()
        }
        .resolve()
        .unwrap();
        assert_eq!(
            config.picture_description_preset.as_deref(),
            Some("smolvlm")
        );
    }

    #[test]
    fn empty_picture_description_preset_resolves_to_none() {
        let config = TaskConfigInput {
            picture_description_preset: Some("   ".to_string()),
            ..TaskConfigInput::default()
        }
        .resolve()
        .unwrap();
        assert!(config.picture_description_preset.is_none());
    }

    #[test]
    fn multipart_parses_picture_description_preset() {
        let fields = HashMap::from([(
            "picture_description_preset".to_string(),
            "granite_vision".to_string(),
        )]);
        let config = TaskConfigInput::from_multipart_fields(&fields)
            .unwrap()
            .resolve()
            .unwrap();
        assert_eq!(
            config.picture_description_preset.as_deref(),
            Some("granite_vision")
        );
    }
}
