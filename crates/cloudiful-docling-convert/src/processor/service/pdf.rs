use std::collections::BTreeMap;
use std::future::Future;

use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::Value;

use super::super::chunking::build_pdf_chunk_plan;
use super::DocumentConverter;
use crate::api::DoclingConvertRequest;
use crate::document::{
    GenericFileConvertOptions, InputDocument, InputKind, OutputFormat, PdfConvertOptions,
};
use crate::error::{PdfConvertError, Result};
use crate::models::{ChunkMetadata, PdfInfo};

impl DocumentConverter {
    pub(super) async fn convert_generic(
        &self,
        input: &InputDocument,
        options: &GenericFileConvertOptions,
        output_formats: &[OutputFormat],
    ) -> Result<crate::document::ConvertedDocument> {
        let raw_result = self
            .docling_client
            .convert_file(
                input,
                &DoclingConvertRequest {
                    output_formats: output_formats.to_vec(),
                    page_range: None,
                    chunking: options.chunking,
                },
            )
            .await?;

        let chunk = Self::chunk_from_result(None, raw_result);
        Ok(Self::assemble_document(
            input,
            input.kind()?,
            None,
            Vec::new(),
            vec![chunk],
        ))
    }

    pub(super) async fn convert_pdf<F, Fut>(
        &self,
        input: &InputDocument,
        options: &PdfConvertOptions,
        output_formats: &[OutputFormat],
        on_progress: &mut F,
    ) -> Result<crate::document::ConvertedDocument>
    where
        F: FnMut(usize, usize) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        let pdf_info = PdfInfo::load_from_bytes(input.bytes.as_ref())?;
        let chunk_plan = build_pdf_chunk_plan(options, &pdf_info);
        let results = self
            .convert_pdf_chunks(input, options, output_formats, &chunk_plan, on_progress)
            .await?;

        let chunks = chunk_plan
            .into_iter()
            .zip(results)
            .map(|(metadata, raw_result)| Self::chunk_from_result(Some(metadata), raw_result))
            .collect();

        Ok(Self::assemble_document(
            input,
            InputKind::Pdf,
            Some(pdf_info.total_pages),
            pdf_info.outlines,
            chunks,
        ))
    }

    async fn convert_pdf_chunks<F, Fut>(
        &self,
        input: &InputDocument,
        options: &PdfConvertOptions,
        output_formats: &[OutputFormat],
        chunk_plan: &[ChunkMetadata],
        on_progress: &mut F,
    ) -> Result<Vec<Value>>
    where
        F: FnMut(usize, usize) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        let mut next_submit_idx = 0;
        let mut active_tasks = FuturesUnordered::new();
        let mut completed_results = BTreeMap::new();

        loop {
            while active_tasks.len() < options.batch_size && next_submit_idx < chunk_plan.len() {
                let idx = next_submit_idx;
                let metadata = chunk_plan[idx].clone();
                let client = self.docling_client.clone();
                let input = input.clone();
                let output_formats = output_formats.to_vec();
                let chunking = options.chunking;

                active_tasks.push(async move {
                    let task_id = client
                        .submit_file_async(
                            &input,
                            &DoclingConvertRequest {
                                output_formats,
                                page_range: Some((metadata.start_page, metadata.end_page)),
                                chunking,
                            },
                        )
                        .await?;
                    let result = client.wait_for_result(&task_id).await?;
                    Ok::<(usize, Value), PdfConvertError>((idx, result))
                });

                next_submit_idx += 1;
            }

            if active_tasks.is_empty() && next_submit_idx >= chunk_plan.len() {
                break;
            }

            if let Some(result) = active_tasks.next().await {
                let (idx, value) = result?;
                completed_results.insert(idx, value);
                on_progress(completed_results.len(), chunk_plan.len()).await;
            }
        }

        Ok((0..chunk_plan.len())
            .filter_map(|idx| completed_results.remove(&idx))
            .collect())
    }
}
