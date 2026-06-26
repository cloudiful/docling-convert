use crate::api::DoclingClient;
use crate::document::{
    ConvertOptions, ConvertRequest, ConvertedDocument, ConvertedFile, FileConvertRequest, InputKind,
};
use crate::error::{PdfConvertError, Result};
use std::future::Future;

mod output;
mod pdf;
mod text;

#[cfg(test)]
mod tests;

pub struct DocumentConverter {
    docling_client: DoclingClient,
}

impl DocumentConverter {
    pub fn new(docling_client: DoclingClient) -> Self {
        Self { docling_client }
    }

    pub async fn convert(&self, request: ConvertRequest) -> Result<ConvertedDocument> {
        self.convert_with_progress(request, |_, _| async {}).await
    }

    pub async fn convert_with_progress<F, Fut>(
        &self,
        request: ConvertRequest,
        mut on_progress: F,
    ) -> Result<ConvertedDocument>
    where
        F: FnMut(usize, usize) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        let input_kind = request.validate()?;

        match (&request.options, input_kind) {
            (ConvertOptions::Text(options), InputKind::Text) => {
                let document =
                    self.convert_text(&request.input, options, &request.output_formats)?;
                on_progress(1, 1).await;
                Ok(document)
            }
            (ConvertOptions::Generic(options), InputKind::Docx | InputKind::Markdown) => {
                let document = self
                    .convert_generic(&request.input, options, &request.output_formats)
                    .await?;
                on_progress(1, 1).await;
                Ok(document)
            }
            (ConvertOptions::Pdf(options), InputKind::Pdf) => {
                self.convert_pdf(
                    &request.input,
                    options,
                    &request.output_formats,
                    &mut on_progress,
                )
                .await
            }
            _ => Err(PdfConvertError::validation_error(
                "request",
                "input kind and convert options do not match",
            )),
        }
    }

    pub async fn convert_to_file(&self, request: FileConvertRequest) -> Result<ConvertedFile> {
        self.convert_to_file_with_progress(request, |_, _| async {})
            .await
    }

    pub async fn convert_to_file_with_progress<F, Fut>(
        &self,
        request: FileConvertRequest,
        on_progress: F,
    ) -> Result<ConvertedFile>
    where
        F: FnMut(usize, usize) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        let document = self
            .convert_with_progress(request.request.clone(), on_progress)
            .await?;
        let output_path = Self::calculate_output_path(
            &request.output_dir,
            &document.filename,
            request.selected_output,
        );

        if !request.overwrite && output_path.exists() {
            return Err(PdfConvertError::operation_error(
                "writing output",
                format!(
                    "output already exists and overwrite is disabled: {}",
                    output_path.display()
                ),
            ));
        }

        Self::write_output_file(&output_path, &document, request.selected_output).await?;

        Ok(ConvertedFile {
            document,
            output_paths: vec![output_path],
        })
    }
}
