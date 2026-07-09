use crate::error::{PdfConvertError, Result};

use super::docling::DoclingConfig;

const OPTIONAL_VLM_FIELDS: [&str; 5] = [
    "docling_openai_base_url",
    "docling_vlm_pipeline_model",
    "docling_picture_description_model",
    "docling_code_formula_model",
    "docling_api_key",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedVlmConfig {
    pub openai_base_url: String,
    pub vlm_pipeline_model: String,
    pub picture_description_model: String,
    pub code_formula_model: String,
    pub api_key: String,
}

impl DoclingConfig {
    pub fn without_vlm(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            openai_base_url: String::new(),
            vlm_pipeline_model: String::new(),
            picture_description_model: String::new(),
            code_formula_model: String::new(),
            api_key: None,
        }
    }

    pub(crate) fn resolved_vlm_config(&self) -> Result<Option<ResolvedVlmConfig>> {
        let openai_base_url = trimmed_non_empty(&self.openai_base_url);
        let vlm_pipeline_model = trimmed_non_empty(&self.vlm_pipeline_model);
        let picture_description_model = trimmed_non_empty(&self.picture_description_model);
        let code_formula_model = trimmed_non_empty(&self.code_formula_model);
        let api_key = self.api_key.as_deref().and_then(trimmed_non_empty);

        let fields = [
            openai_base_url.as_ref(),
            vlm_pipeline_model.as_ref(),
            picture_description_model.as_ref(),
            code_formula_model.as_ref(),
            api_key.as_ref(),
        ];
        let present_count = fields.iter().filter(|value| value.is_some()).count();

        if present_count == 0 {
            return Ok(None);
        }

        if present_count != fields.len() {
            return Err(PdfConvertError::validation_error(
                "docling runtime",
                format!(
                    "optional VLM runtime config is incomplete; provide all of: {}, or leave all unset",
                    OPTIONAL_VLM_FIELDS.join(", ")
                ),
            ));
        }

        Ok(Some(ResolvedVlmConfig {
            openai_base_url: openai_base_url.expect("openai_base_url present"),
            vlm_pipeline_model: vlm_pipeline_model.expect("vlm_pipeline_model present"),
            picture_description_model: picture_description_model
                .expect("picture_description_model present"),
            code_formula_model: code_formula_model.expect("code_formula_model present"),
            api_key: api_key.expect("api_key present"),
        }))
    }
}

fn trimmed_non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> DoclingConfig {
        DoclingConfig {
            base_url: "http://127.0.0.1:5001/v1".into(),
            openai_base_url: String::new(),
            vlm_pipeline_model: String::new(),
            picture_description_model: String::new(),
            code_formula_model: String::new(),
            api_key: None,
        }
    }

    #[test]
    fn missing_vlm_bundle_is_allowed() {
        assert!(config().resolved_vlm_config().unwrap().is_none());
    }

    #[test]
    fn partial_vlm_bundle_is_rejected() {
        let mut config = config();
        config.openai_base_url = "https://api.openai.com/v1".into();
        config.vlm_pipeline_model = "gpt-4o-mini".into();

        let error = config.resolved_vlm_config().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("optional VLM runtime config is incomplete")
        );
    }
}
