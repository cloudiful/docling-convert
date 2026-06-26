use crate::document::PdfConvertOptions;
use crate::models::{ChunkMetadata, PdfInfo};

pub fn build_pdf_chunk_plan(options: &PdfConvertOptions, pdf_info: &PdfInfo) -> Vec<ChunkMetadata> {
    if options.split_by_bookmark {
        pdf_info.calculate_hybrid_ranges(options.pages_per_file)
    } else if options.split_input {
        pdf_info.calculate_page_ranges(options.pages_per_file)
    } else {
        vec![ChunkMetadata {
            start_page: 1,
            end_page: pdf_info.total_pages,
            bookmark_index: None,
            bookmark_title: None,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_chunk_plan_stays_pdf_specific() {
        let pdf_info = PdfInfo {
            total_pages: 10,
            outlines: Vec::new(),
        };
        let plan = build_pdf_chunk_plan(
            &PdfConvertOptions {
                pages_per_file: 4,
                split_input: true,
                split_by_bookmark: false,
                chunking: false,
                batch_size: 2,
            },
            &pdf_info,
        );

        let page_ranges: Vec<(u32, u32)> = plan
            .into_iter()
            .map(|chunk| (chunk.start_page, chunk.end_page))
            .collect();
        assert_eq!(page_ranges, vec![(1, 4), (5, 8), (9, 10)]);
    }
}
