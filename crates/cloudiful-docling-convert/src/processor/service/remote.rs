use super::DocumentConverter;
use crate::api::{DoclingConvertRequest, DoclingResult};
use crate::document::{InputDocument, OutputFormat, RemoteConvertOptions};
use crate::error::{PdfConvertError, Result};
use crate::models::TaskStatusResponse;

impl DocumentConverter {
    pub async fn convert_source_to_file_async(
        &self,
        url: &str,
        filename: impl Into<String>,
        input_kind: crate::document::InputKind,
        output_formats: Vec<OutputFormat>,
        options: &RemoteConvertOptions,
        output_dir: std::path::PathBuf,
        selected_output: OutputFormat,
        overwrite: bool,
    ) -> Result<crate::document::ConvertedFile> {
        self.convert_source_to_file_async_with_docling_progress(
            url,
            filename,
            input_kind,
            output_formats,
            options,
            output_dir,
            selected_output,
            overwrite,
            |_| async {},
        )
        .await
    }

    pub async fn convert_source_to_file_async_with_docling_progress<F, Fut>(
        &self,
        url: &str,
        filename: impl Into<String>,
        input_kind: crate::document::InputKind,
        output_formats: Vec<OutputFormat>,
        options: &RemoteConvertOptions,
        output_dir: std::path::PathBuf,
        selected_output: OutputFormat,
        overwrite: bool,
        mut on_status: F,
    ) -> Result<crate::document::ConvertedFile>
    where
        F: FnMut(TaskStatusResponse) -> Fut + Send,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let filename = filename.into();
        let input = InputDocument::new(
            filename,
            input_kind.canonical_media_type("source", None),
            Vec::<u8>::new(),
        )
        .with_input_kind(input_kind);
        let request = DoclingConvertRequest {
            output_formats,
            page_range: None,
            chunker: options.chunker,
            chunking: options.chunking.clone(),
            pipeline: options.pipeline,
        };
        let task_id = self
            .docling_client
            .submit_source_async(url, input_kind, &request)
            .await?;
        let task_result = self
            .docling_client
            .wait_for_result_with_progress(&task_id, &mut on_status)
            .await?;
        let document = Self::document_from_task_result(&input, task_result)?;
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
        Ok(crate::document::ConvertedFile {
            document,
            output_paths: vec![output_path],
        })
    }

    pub(super) async fn convert_remote_with_docling_progress<F, Fut>(
        &self,
        input: &InputDocument,
        options: &RemoteConvertOptions,
        output_formats: &[OutputFormat],
        on_status: &mut F,
    ) -> Result<crate::document::ConvertedDocument>
    where
        F: FnMut(TaskStatusResponse) -> Fut + Send,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let request = DoclingConvertRequest {
            output_formats: output_formats.to_vec(),
            page_range: None,
            chunker: options.chunker,
            chunking: options.chunking.clone(),
            pipeline: options.pipeline,
        };
        let task_id = self
            .docling_client
            .submit_file_async(input, &request)
            .await?;
        let task_result = self
            .docling_client
            .wait_for_result_with_progress(&task_id, on_status)
            .await?;
        Self::document_from_task_result(input, task_result)
    }

    pub(super) async fn convert_remote<F, Fut>(
        &self,
        input: &InputDocument,
        options: &RemoteConvertOptions,
        output_formats: &[OutputFormat],
        asynchronous: bool,
        on_progress: &mut F,
    ) -> Result<crate::document::ConvertedDocument>
    where
        F: FnMut(usize, usize) -> Fut + Send,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let request = DoclingConvertRequest {
            output_formats: output_formats.to_vec(),
            page_range: None,
            chunker: options.chunker,
            chunking: options.chunking.clone(),
            pipeline: options.pipeline,
        };

        let result = if asynchronous {
            let task_id = self
                .docling_client
                .submit_file_async(input, &request)
                .await?;
            let task_result = self.docling_client.wait_for_result(&task_id).await?;
            let document = Self::document_from_task_result(input, task_result)?;
            on_progress(1, 1).await;
            return Ok(document);
        } else {
            let result = self.docling_client.convert_file(input, &request).await?;
            on_progress(1, 1).await;
            result
        };

        Self::document_from_result(input, result)
    }

    pub(super) fn document_from_result(
        input: &InputDocument,
        result: DoclingResult,
    ) -> Result<crate::document::ConvertedDocument> {
        let input_kind = input.kind()?;
        match result {
            DoclingResult::Failure(failure) => Err(PdfConvertError::api_task_failed(
                "failure",
                failure.failure.message,
            )),
            result => Ok(Self::build_document(input, input_kind, result)),
        }
    }

    pub fn document_from_task_result(
        input: &InputDocument,
        task_result: crate::api::DoclingTaskResult,
    ) -> Result<crate::document::ConvertedDocument> {
        let mut document = Self::document_from_result(input, task_result.result)?;
        document.errors.extend(task_result.errors);
        document.errors.sort();
        document.errors.dedup();
        Ok(document)
    }
}
