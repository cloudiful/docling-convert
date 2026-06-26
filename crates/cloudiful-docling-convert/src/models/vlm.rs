use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub struct OpenRouterConfigBuilder;

impl OpenRouterConfigBuilder {
    pub fn chat_completions_url(base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        format!("{}/chat/completions", trimmed)
    }

    pub fn headers(api_key: &str) -> Value {
        json!({
            "Authorization": format!("Bearer {}", api_key),
        })
    }

    pub fn engine_options(
        base_url: &str,
        api_key: &str,
        model_name: &str,
        timeout: i32,
        concurrency: i32,
    ) -> BaseVlmEngineOptions {
        BaseVlmEngineOptions {
            engine_type: "api_openai".to_string(),
            url: Self::chat_completions_url(base_url),
            headers: Some(Self::headers(api_key)),
            params: Some(json!({ "model": model_name })),
            timeout: Some(timeout),
            concurrency: Some(concurrency),
        }
    }

    pub fn model_spec(model_name: &str, prompt: &str, max_new_tokens: i32) -> VlmModelSpec {
        VlmModelSpec {
            name: Some(model_name.to_owned()),
            default_repo_id: Some(model_name.to_owned()),
            revision: None,
            prompt: prompt.to_string(),
            response_format: "markdown".to_string(),
            max_new_tokens: Some(max_new_tokens),
            supported_engines: None,
            engine_overrides: None,
            api_overrides: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VlmModelSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_repo_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    pub prompt: String,
    pub response_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_new_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_engines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_overrides: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_overrides: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BaseVlmEngineOptions {
    pub engine_type: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<i32>,
}

pub type PictureClassificationLabel = String;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PictureDescriptionVlmEngineOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture_area_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_allow: Option<Vec<PictureClassificationLabel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_deny: Option<Vec<PictureClassificationLabel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_min_confidence: Option<f64>,
    pub engine_options: BaseVlmEngineOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<Value>,
    pub model_spec: VlmModelSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

impl PictureDescriptionVlmEngineOptions {
    #[allow(dead_code)]
    pub fn openrouter(api_key: &str, model_name: &str) -> Self {
        const PICTURE_DESCRIPTION_PROMPT: &str = "Describe this image in a few sentences.";
        Self::for_openai_compatible(
            "https://openrouter.ai/api/v1",
            api_key,
            model_name,
            PICTURE_DESCRIPTION_PROMPT,
            300,
            60,
        )
    }

    pub fn for_openai_compatible(
        base_url: &str,
        api_key: &str,
        model_name: &str,
        prompt: &str,
        max_tokens: i32,
        timeout: i32,
    ) -> Self {
        Self {
            batch_size: None,
            scale: Some(1.0),
            picture_area_threshold: None,
            classification_allow: None,
            classification_deny: None,
            classification_min_confidence: None,
            engine_options: OpenRouterConfigBuilder::engine_options(
                base_url, api_key, model_name, timeout, 2,
            ),
            generation_config: None,
            model_spec: OpenRouterConfigBuilder::model_spec(model_name, prompt, max_tokens),
            prompt: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CodeFormulaVlmOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extract_code: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extract_formulas: Option<bool>,
    pub engine_options: BaseVlmEngineOptions,
    pub model_spec: VlmModelSpec,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VlmConvertOptions {
    pub engine_options: BaseVlmEngineOptions,
    pub model_spec: VlmModelSpec,
    pub scale: Option<f64>,
    pub max_size: Option<i32>,
    pub batch_size: Option<i32>,
    pub force_backend_text: bool,
}
