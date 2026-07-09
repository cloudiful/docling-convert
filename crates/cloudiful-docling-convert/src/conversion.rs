use crate::api::{DoclingClient, DoclingConfig};
use crate::document::{
    ConvertOptions, GenericFileConvertOptions, InputDocument, InputKind, PdfConvertOptions,
    TextConvertOptions,
};
use crate::error::{PdfConvertError, Result};
use crate::pdf::PdfInfo;
use crate::processor::build_pdf_chunk_plan;

#[derive(Debug, Clone)]
pub struct DoclingRuntimeConfig {
    pub docling_base_url: String,
    pub openai_base_url: String,
    pub vlm_pipeline_model: String,
    pub picture_description_model: String,
    pub code_formula_model: String,
    pub api_key: Option<String>,
}

impl DoclingRuntimeConfig {
    pub fn without_vlm(docling_base_url: impl Into<String>) -> Self {
        Self {
            docling_base_url: docling_base_url.into(),
            openai_base_url: String::new(),
            vlm_pipeline_model: String::new(),
            picture_description_model: String::new(),
            code_formula_model: String::new(),
            api_key: None,
        }
    }

    pub fn into_docling_config(self) -> DoclingConfig {
        DoclingConfig {
            base_url: self.docling_base_url,
            openai_base_url: self.openai_base_url,
            vlm_pipeline_model: self.vlm_pipeline_model,
            picture_description_model: self.picture_description_model,
            code_formula_model: self.code_formula_model,
            api_key: self.api_key,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConversionBehavior {
    pub pages_per_file: u32,
    pub split_input: bool,
    pub split_by_bookmark: bool,
    pub chunking: bool,
    pub batch_size: usize,
}

impl Default for ConversionBehavior {
    fn default() -> Self {
        Self {
            pages_per_file: 5,
            split_input: true,
            split_by_bookmark: false,
            chunking: false,
            batch_size: 2,
        }
    }
}

pub fn build_docling_client(config: DoclingRuntimeConfig) -> Result<DoclingClient> {
    DoclingClient::new(config.into_docling_config())
}

pub fn build_convert_options(
    input_kind: InputKind,
    behavior: &ConversionBehavior,
) -> Result<ConvertOptions> {
    match input_kind {
        InputKind::Pdf => Ok(ConvertOptions::Pdf(PdfConvertOptions {
            pages_per_file: behavior.pages_per_file,
            split_input: behavior.split_input,
            split_by_bookmark: behavior.split_by_bookmark,
            chunking: behavior.chunking,
            batch_size: behavior.batch_size,
        })),
        InputKind::Docx | InputKind::Markdown => {
            reject_pdf_only_options(input_kind, behavior)?;
            Ok(ConvertOptions::Generic(GenericFileConvertOptions {
                chunking: behavior.chunking,
            }))
        }
        InputKind::Text => {
            reject_pdf_only_options(input_kind, behavior)?;
            Ok(ConvertOptions::Text(TextConvertOptions::default()))
        }
    }
}

pub fn build_pdf_options(behavior: &ConversionBehavior) -> Result<PdfConvertOptions> {
    let options = PdfConvertOptions {
        pages_per_file: behavior.pages_per_file,
        split_input: behavior.split_input,
        split_by_bookmark: behavior.split_by_bookmark,
        chunking: behavior.chunking,
        batch_size: behavior.batch_size,
    };
    options.validate()?;
    Ok(options)
}

pub fn count_input_chunks(input: &InputDocument, behavior: &ConversionBehavior) -> Result<usize> {
    match input.kind()? {
        InputKind::Pdf => {
            let pdf_info = PdfInfo::load_from_bytes(input.bytes.as_ref())?;
            let total = build_pdf_chunk_plan(&build_pdf_options(behavior)?, &pdf_info)
                .len()
                .max(1);
            Ok(total)
        }
        _ => Ok(1),
    }
}

fn reject_pdf_only_options(input_kind: InputKind, behavior: &ConversionBehavior) -> Result<()> {
    if behavior.split_by_bookmark {
        return Err(PdfConvertError::validation_error(
            "split_by_bookmark",
            format!(
                "bookmark splitting is only available for PDF inputs, got {:?}",
                input_kind
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_input_rejects_bookmark_splitting() {
        let err = build_convert_options(
            InputKind::Docx,
            &ConversionBehavior {
                split_by_bookmark: true,
                ..ConversionBehavior::default()
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("bookmark splitting"));
    }
}
