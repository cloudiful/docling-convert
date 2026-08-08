use crate::api::DoclingClient;
use crate::document::{
    ConvertOptions, ConvertRequest, ConvertedDocument, ConvertedFile, FileConvertRequest, InputKind,
};
use crate::error::{PdfConvertError, Result};
use crate::models::TaskStatusResponse;
use std::future::Future;

mod output;
mod remote;
mod text;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct DocumentConverter {
    pub(crate) docling_client: DoclingClient,
}

impl DocumentConverter {
    pub fn new(docling_client: DoclingClient) -> Self {
        Self { docling_client }
    }

    /// Submit a whole-document asynchronous conversion and return only the remote
    /// task id. The caller owns polling and result fetching, which makes the
    /// conversion resumable across restarts.
    pub async fn submit_async(&self, request: &ConvertRequest) -> Result<String> {
        let input_kind = request.validate()?;
        let options = match &request.options {
            ConvertOptions::Pdf(options) | ConvertOptions::Generic(options) => options,
            ConvertOptions::Text(_) => {
                return Err(PdfConvertError::validation_error(
                    "request",
                    "text inputs are converted locally and cannot be submitted to Docling",
                ));
            }
        };
        if matches!(input_kind, InputKind::Text) {
            return Err(PdfConvertError::validation_error(
                "request",
                "text inputs are converted locally and cannot be submitted to Docling",
            ));
        }
        let remote_request = crate::api::DoclingConvertRequest {
            output_formats: request.output_formats.clone(),
            page_range: None,
            chunker: options.chunker,
            chunking: options.chunking.clone(),
            pipeline: options.pipeline,
        };
        self.docling_client
            .submit_file_async(&request.input, &remote_request)
            .await
    }

    pub async fn convert(&self, request: ConvertRequest) -> Result<ConvertedDocument> {
        self.convert_with_progress(request, |_, _| async {}).await
    }

    pub async fn convert_async(&self, request: ConvertRequest) -> Result<ConvertedDocument> {
        self.convert_async_with_progress(request, |_, _| async {})
            .await
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
        self.convert_internal(request, false, &mut on_progress)
            .await
    }

    pub async fn convert_async_with_progress<F, Fut>(
        &self,
        request: ConvertRequest,
        mut on_progress: F,
    ) -> Result<ConvertedDocument>
    where
        F: FnMut(usize, usize) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        self.convert_internal(request, true, &mut on_progress).await
    }

    pub async fn convert_async_with_docling_progress<F, Fut>(
        &self,
        request: ConvertRequest,
        mut on_status: F,
    ) -> Result<ConvertedDocument>
    where
        F: FnMut(TaskStatusResponse) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        let input_kind = request.validate()?;
        match (&request.options, input_kind) {
            (ConvertOptions::Text(options), InputKind::Text) => {
                self.convert_text(&request.input, options, &request.output_formats)
            }
            (ConvertOptions::Generic(options), _) if input_kind.uses_generic_convert_options() => {
                self.convert_remote_with_docling_progress(
                    &request.input,
                    options,
                    &request.output_formats,
                    &mut on_status,
                )
                .await
            }
            (ConvertOptions::Pdf(options), InputKind::Pdf) => {
                self.convert_remote_with_docling_progress(
                    &request.input,
                    options,
                    &request.output_formats,
                    &mut on_status,
                )
                .await
            }
            _ => Err(PdfConvertError::validation_error(
                "request",
                "input kind and convert options do not match",
            )),
        }
    }

    async fn convert_internal<F, Fut>(
        &self,
        request: ConvertRequest,
        asynchronous: bool,
        on_progress: &mut F,
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
            (ConvertOptions::Generic(options), _) if input_kind.uses_generic_convert_options() => {
                self.convert_remote(
                    &request.input,
                    options,
                    &request.output_formats,
                    asynchronous,
                    on_progress,
                )
                .await
            }
            (ConvertOptions::Pdf(options), InputKind::Pdf) => {
                self.convert_remote(
                    &request.input,
                    options,
                    &request.output_formats,
                    asynchronous,
                    on_progress,
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
        self.convert_to_file_internal(request, false, on_progress)
            .await
    }

    pub async fn convert_to_file_async(
        &self,
        request: FileConvertRequest,
    ) -> Result<ConvertedFile> {
        self.convert_to_file_async_with_progress(request, |_, _| async {})
            .await
    }

    pub async fn convert_to_file_async_with_progress<F, Fut>(
        &self,
        request: FileConvertRequest,
        on_progress: F,
    ) -> Result<ConvertedFile>
    where
        F: FnMut(usize, usize) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        self.convert_to_file_internal(request, true, on_progress)
            .await
    }

    pub async fn convert_to_file_async_with_docling_progress<F, Fut>(
        &self,
        request: FileConvertRequest,
        on_status: F,
    ) -> Result<ConvertedFile>
    where
        F: FnMut(TaskStatusResponse) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        let selected_output = request.selected_output;
        let output_dir = request.output_dir.clone();
        let overwrite = request.overwrite;
        let document = self
            .convert_async_with_docling_progress(request.request, on_status)
            .await?;
        let output_path =
            Self::calculate_output_path(&output_dir, &document.filename, selected_output);

        if !overwrite && output_path.exists() {
            return Err(PdfConvertError::operation_error(
                "writing output",
                format!(
                    "output already exists and overwrite is disabled: {}",
                    output_path.display()
                ),
            ));
        }

        Self::write_output_file(&output_path, &document, selected_output).await?;
        Ok(ConvertedFile {
            document,
            output_paths: vec![output_path],
        })
    }

    async fn convert_to_file_internal<F, Fut>(
        &self,
        request: FileConvertRequest,
        asynchronous: bool,
        on_progress: F,
    ) -> Result<ConvertedFile>
    where
        F: FnMut(usize, usize) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        let selected_output = request.selected_output;
        let output_dir = request.output_dir.clone();
        let overwrite = request.overwrite;
        let document = if asynchronous {
            self.convert_async_with_progress(request.request, on_progress)
                .await?
        } else {
            self.convert_with_progress(request.request, on_progress)
                .await?
        };
        let output_path =
            Self::calculate_output_path(&output_dir, &document.filename, selected_output);

        if !overwrite && output_path.exists() {
            return Err(PdfConvertError::operation_error(
                "writing output",
                format!(
                    "output already exists and overwrite is disabled: {}",
                    output_path.display()
                ),
            ));
        }

        Self::write_output_file(&output_path, &document, selected_output).await?;
        Ok(ConvertedFile {
            document,
            output_paths: vec![output_path],
        })
    }
}
