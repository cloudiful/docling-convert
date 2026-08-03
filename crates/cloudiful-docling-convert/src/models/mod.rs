pub mod api;
pub mod vlm;

pub use api::{
    ChunkDocumentResponse, ConversionStatus, ConvertDocumentResponse, DoclingChunk,
    DoclingErrorItem, DocumentResultItem, ExportDocumentResponse, PublicFailureInfo,
    TaskFailureResult, TaskPostResponse, TaskProcessingMeta, TaskStatusResponse,
};
pub use vlm::{CodeFormulaVlmOptions, PictureDescriptionVlmEngineOptions};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vlm_options_for_openai_compatible_builds_expected_config() {
        let api_key = "test_key";
        let model = "test_model";
        let opts = PictureDescriptionVlmEngineOptions::for_openai_compatible(
            "https://example.com/v1/",
            api_key,
            model,
            "Describe this image in a few sentences.",
            300,
            60,
        );

        assert_eq!(opts.engine_options.engine_type, "api_openai");
        assert_eq!(
            opts.engine_options.url,
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(opts.model_spec.name.as_deref(), Some(model));
        assert!(opts.engine_options.headers.is_some());
    }
}
