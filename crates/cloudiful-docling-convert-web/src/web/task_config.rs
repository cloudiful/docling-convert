use std::collections::HashMap;

use axum::http::StatusCode;
use serde::Deserialize;

use super::state::TaskConfig;
use super::support::parse_output_format;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TaskConfigInput {
    pub format: Option<String>,
    pub pages_per_file: Option<u32>,
    pub split_input: Option<bool>,
    pub split_by_bookmark: Option<bool>,
    pub chunking: Option<bool>,
    pub batch_size: Option<usize>,
}

impl TaskConfigInput {
    pub fn resolve(self) -> Result<TaskConfig, StatusCode> {
        let mut config = TaskConfig::default();

        if let Some(format) = self.format {
            parse_output_format(&format).map_err(|_| StatusCode::BAD_REQUEST)?;
            config.format = format;
        }

        if let Some(pages_per_file) = self.pages_per_file {
            if pages_per_file == 0 {
                return Err(StatusCode::BAD_REQUEST);
            }
            config.pages_per_file = pages_per_file;
        }

        if let Some(batch_size) = self.batch_size {
            if batch_size == 0 {
                return Err(StatusCode::BAD_REQUEST);
            }
            config.batch_size = batch_size;
        }

        if let Some(split_input) = self.split_input {
            config.split_input = split_input;
        }
        if let Some(split_by_bookmark) = self.split_by_bookmark {
            config.split_by_bookmark = split_by_bookmark;
        }
        if let Some(chunking) = self.chunking {
            config.chunking = chunking;
        }

        Ok(config)
    }

    pub fn from_multipart_fields(fields: &HashMap<String, String>) -> Result<Self, StatusCode> {
        Ok(Self {
            format: fields.get("format").cloned(),
            pages_per_file: parse_optional(fields, "pages_per_file")?,
            split_input: parse_optional_bool(fields, "split_input")?,
            split_by_bookmark: parse_optional_bool(fields, "split_by_bookmark")?,
            chunking: parse_optional_bool(fields, "chunking")?,
            batch_size: parse_optional(fields, "batch_size")?,
        })
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
    fn resolves_defaults_when_empty() {
        let config = TaskConfigInput::default().resolve().unwrap();
        assert_eq!(config, TaskConfig::default());
    }

    #[test]
    fn parses_multipart_values() {
        let fields = HashMap::from([
            ("format".to_string(), "json".to_string()),
            ("pages_per_file".to_string(), "9".to_string()),
            ("split_input".to_string(), "false".to_string()),
            ("split_by_bookmark".to_string(), "true".to_string()),
            ("chunking".to_string(), "true".to_string()),
            ("batch_size".to_string(), "4".to_string()),
        ]);

        let config = TaskConfigInput::from_multipart_fields(&fields)
            .unwrap()
            .resolve()
            .unwrap();

        assert_eq!(config.format, "json");
        assert_eq!(config.pages_per_file, 9);
        assert!(!config.split_input);
        assert!(config.split_by_bookmark);
        assert!(config.chunking);
        assert_eq!(config.batch_size, 4);
    }
}
