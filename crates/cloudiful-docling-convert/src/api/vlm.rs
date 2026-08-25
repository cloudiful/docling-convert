use reqwest::multipart;

use crate::error::Result;
use crate::models::vlm::{OpenRouterConfigBuilder, VlmConvertOptions};
use crate::models::{CodeFormulaVlmOptions, PictureDescriptionVlmEngineOptions};

use super::docling::DoclingClient;
use super::vlm_config::ResolvedVlmConfig;

impl DoclingClient {
    pub(super) fn apply_vlm_config(
        &self,
        mut form: multipart::Form,
        vlm_config: &ResolvedVlmConfig,
        picture_description_preset: Option<&str>,
    ) -> Result<multipart::Form> {
        let code_formula_custom_config = CodeFormulaVlmOptions {
            scale: Some(2.0),
            max_size: None,
            extract_code: Some(true),
            extract_formulas: Some(true),
            engine_options: OpenRouterConfigBuilder::engine_options(
                &vlm_config.openai_base_url,
                &vlm_config.api_key,
                &vlm_config.code_formula_model,
                30,
                2,
            ),
            model_spec: OpenRouterConfigBuilder::model_spec(
                &vlm_config.code_formula_model,
                "Recognize code blocks and mathematical formulas in the image. For code, output the full code; for mathematical formulas, output in LaTeX format.",
                1000,
            ),
        };
        let vlm_pipeline_custom_config = VlmConvertOptions {
            engine_options: OpenRouterConfigBuilder::engine_options(
                &vlm_config.openai_base_url,
                &vlm_config.api_key,
                &vlm_config.vlm_pipeline_model,
                30,
                2,
            ),
            model_spec: OpenRouterConfigBuilder::model_spec(
                &vlm_config.vlm_pipeline_model,
                "",
                1000,
            ),
            scale: Some(1.0),
            max_size: None,
            batch_size: None,
            force_backend_text: true,
        };

        form = form.text(
            "vlm_pipeline_custom_config",
            serde_json::to_string(&vlm_pipeline_custom_config)?,
        );
        // When a `picture_description_preset` is selected we suppress the
        // legacy `picture_description_custom_config` form field because the
        // preset is mutually exclusive with it on Docling Serve. Without a
        // preset, the legacy custom VLM configuration is preserved unchanged.
        if picture_description_preset.is_none() {
            let picture_description_custom_config =
                PictureDescriptionVlmEngineOptions::for_openai_compatible(
                    &vlm_config.openai_base_url,
                    &vlm_config.api_key,
                    &vlm_config.picture_description_model,
                    "Describe this image in a few sentences.",
                    300,
                    60,
                );
            form = form.text(
                "picture_description_custom_config",
                serde_json::to_string(&picture_description_custom_config)?,
            );
        }
        form = form.text(
            "code_formula_custom_config",
            serde_json::to_string(&code_formula_custom_config)?,
        );
        form = form.text("do_code_enrichment", "true");
        form = form.text("do_formula_enrichment", "true");
        form = form.text("do_picture_description", "true");
        form = form.text("ocr_engine", "rapidocr");
        form = form.text("image_export_mode", "placeholder");
        Ok(form)
    }
}
