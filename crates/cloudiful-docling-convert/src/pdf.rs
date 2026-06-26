use std::collections::BTreeMap;
use std::path::Path;

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::error::{PdfConvertError, Result};
pub use crate::models::{Bookmark, ChunkMetadata, PdfInfo};

impl PdfInfo {
    pub fn load(pdf_path: &Path) -> Result<Self> {
        let doc = Document::load(pdf_path)
            .map_err(|e| PdfConvertError::parse_error("PDF document", e.to_string()))?;
        Ok(Self::from_document(doc))
    }

    pub fn load_from_bytes(data: &[u8]) -> Result<Self> {
        let doc = Document::load_mem(data)
            .map_err(|e| PdfConvertError::parse_error("PDF document", e.to_string()))?;
        Ok(Self::from_document(doc))
    }

    fn from_document(doc: Document) -> Self {
        let total_pages = doc.get_pages().len() as u32;
        let outlines = Self::extract_outlines(&doc);

        PdfInfo {
            total_pages,
            outlines,
        }
    }

    fn extract_outlines(doc: &Document) -> Vec<Bookmark> {
        let mut outlines = Vec::new();
        if let Ok(catalog) = doc.catalog() {
            if let Ok(Object::Reference(outlines_ref)) = catalog.get(b"Outlines") {
                if let Ok(outlines_dict) =
                    doc.get_object(*outlines_ref).and_then(|obj| obj.as_dict())
                {
                    if let Ok(Object::Reference(first_ref)) = outlines_dict.get(b"First") {
                        let page_map = Self::build_page_map(&doc);
                        Self::traverse_outlines(&doc, *first_ref, 0, &page_map, &mut outlines);
                    }
                }
            }
        }

        outlines
    }

    fn build_page_map(doc: &Document) -> BTreeMap<ObjectId, u32> {
        let mut page_map = BTreeMap::new();
        for (idx, &page_id) in doc.get_pages().values().enumerate() {
            page_map.insert(page_id, (idx + 1) as u32);
        }
        page_map
    }

    fn traverse_outlines(
        doc: &Document,
        current_ref: ObjectId,
        level: usize,
        page_map: &BTreeMap<ObjectId, u32>,
        outlines: &mut Vec<Bookmark>,
    ) {
        let mut current_obj_ref = Some(current_ref);
        while let Some(obj_ref) = current_obj_ref {
            if let Ok(dict) = doc.get_object(obj_ref).and_then(|obj| obj.as_dict()) {
                if let Some(bookmark) = Self::parse_bookmark(doc, dict, level, page_map) {
                    outlines.push(bookmark);
                }

                if let Ok(Object::Reference(first_child)) = dict.get(b"First") {
                    Self::traverse_outlines(doc, *first_child, level + 1, page_map, outlines);
                }

                current_obj_ref = match dict.get(b"Next") {
                    Ok(Object::Reference(next_ref)) => Some(*next_ref),
                    _ => None,
                };
            } else {
                break;
            }
        }
    }

    fn parse_bookmark(
        doc: &Document,
        dict: &Dictionary,
        level: usize,
        page_map: &BTreeMap<ObjectId, u32>,
    ) -> Option<Bookmark> {
        let title = match dict.get(b"Title") {
            Ok(Object::String(s, _)) => String::from_utf8_lossy(s).into_owned(),
            _ => return None,
        };

        let page_number = if let Ok(dest) = dict.get(b"Dest") {
            Self::resolve_dest(doc, dest, page_map)
        } else if let Ok(Object::Dictionary(action)) = dict.get(b"A") {
            if let Ok(Object::Name(name)) = action.get(b"S") {
                if name == b"GoTo" {
                    if let Ok(dest) = action.get(b"D") {
                        Self::resolve_dest(doc, dest, page_map)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        page_number.map(|pn| Bookmark {
            title,
            page_number: pn,
            level,
        })
    }

    fn resolve_dest(
        doc: &Document,
        dest: &Object,
        page_map: &BTreeMap<ObjectId, u32>,
    ) -> Option<u32> {
        let dest = match dest {
            Object::Reference(r) => doc.get_object(*r).ok()?,
            _ => dest,
        };

        match dest {
            Object::Array(arr) => {
                if let Some(Object::Reference(page_ref)) = arr.get(0) {
                    page_map.get(page_ref).copied()
                } else if let Some(Object::Integer(page_idx)) = arr.get(0) {
                    Some((*page_idx as u32) + 1)
                } else {
                    None
                }
            }
            // Named destinations are currently ignored because this parser only resolves
            // inline page refs / page indexes. Keep README in sync with this limitation.
            Object::Name(_) => None,
            _ => None,
        }
    }

    pub fn calculate_hybrid_ranges(&self, pages_per_chunk: u32) -> Vec<ChunkMetadata> {
        if self.outlines.is_empty() {
            return self.calculate_page_ranges(pages_per_chunk);
        }

        let mut boundary_points: Vec<(u32, Option<usize>, Option<String>)> = self
            .outlines
            .iter()
            .enumerate()
            .map(|(idx, b)| (b.page_number, Some(idx + 1), Some(b.title.clone())))
            .collect();

        if !boundary_points.iter().any(|(p, _, _)| *p == 1) {
            boundary_points.push((1, None, None));
        }
        boundary_points.push((self.total_pages + 1, None, None));
        boundary_points.sort_by_key(|(p, _, _)| *p);

        let mut ranges = Vec::new();
        for i in 0..boundary_points.len() - 1 {
            let (start, b_idx, b_title) = &boundary_points[i];
            let (next_boundary, _, _) = &boundary_points[i + 1];

            let mut current = *start;
            while current < *next_boundary {
                let end = std::cmp::min(current + pages_per_chunk - 1, *next_boundary - 1);
                ranges.push(ChunkMetadata {
                    start_page: current,
                    end_page: end,
                    bookmark_index: b_idx.clone(),
                    bookmark_title: b_title.clone(),
                });
                current = end + 1;
            }
        }

        log::debug!("Calculated hybrid page ranges: {:?}", ranges);
        ranges
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn calculate_bookmark_ranges(&self) -> Vec<ChunkMetadata> {
        if self.outlines.is_empty() {
            return vec![ChunkMetadata {
                start_page: 1,
                end_page: self.total_pages,
                bookmark_index: None,
                bookmark_title: None,
            }];
        }

        let mut points: Vec<(u32, usize, String)> = self
            .outlines
            .iter()
            .enumerate()
            .map(|(idx, b)| (b.page_number, idx + 1, b.title.clone()))
            .collect();
        points.sort_by_key(|p| p.0);

        let mut ranges = Vec::new();
        for i in 0..points.len() {
            let (start, b_idx, b_title) = &points[i];
            let end = if i + 1 < points.len() {
                points[i + 1].0 - 1
            } else {
                self.total_pages
            };

            if *start <= end {
                ranges.push(ChunkMetadata {
                    start_page: *start,
                    end_page: end,
                    bookmark_index: Some(*b_idx),
                    bookmark_title: Some(b_title.clone()),
                });
            }
        }

        if ranges.is_empty() {
            ranges.push(ChunkMetadata {
                start_page: 1,
                end_page: self.total_pages,
                bookmark_index: None,
                bookmark_title: None,
            });
        }

        ranges
    }

    pub fn calculate_page_ranges(&self, pages_per_chunk: u32) -> Vec<ChunkMetadata> {
        let mut ranges = Vec::new();

        for start in (0..self.total_pages).step_by(pages_per_chunk as usize) {
            let actual_start = start + 1;
            let actual_end = std::cmp::min(start + pages_per_chunk, self.total_pages);
            ranges.push(ChunkMetadata {
                start_page: actual_start,
                end_page: actual_end,
                bookmark_index: None,
                bookmark_title: None,
            });
        }

        log::debug!("Calculated page ranges: {:?}", ranges);

        ranges
    }
}

#[allow(dead_code)]
pub fn extract_filename(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_page_ranges_even_division() {
        let ranges = calculate_ranges_helper(10, 5);
        assert_eq!(ranges, vec![(1, 5), (6, 10)]);
    }

    #[test]
    fn test_calculate_page_ranges_uneven_division() {
        let ranges = calculate_ranges_helper(12, 5);
        assert_eq!(ranges, vec![(1, 5), (6, 10), (11, 12)]);
    }

    #[test]
    fn test_calculate_page_ranges_single_page() {
        let ranges = calculate_ranges_helper(1, 5);
        assert_eq!(ranges, vec![(1, 1)]);
    }

    #[test]
    fn test_calculate_page_ranges_chunk_larger_than_total() {
        let ranges = calculate_ranges_helper(3, 10);
        assert_eq!(ranges, vec![(1, 3)]);
    }

    #[test]
    fn test_calculate_bookmark_ranges_empty() {
        let info = PdfInfo {
            total_pages: 10,
            outlines: vec![],
        };
        let ranges = info.calculate_bookmark_ranges();
        assert_eq!(
            ranges,
            vec![ChunkMetadata {
                start_page: 1,
                end_page: 10,
                bookmark_index: None,
                bookmark_title: None
            }]
        );
    }

    #[test]
    fn test_calculate_bookmark_ranges_simple() {
        let info = PdfInfo {
            total_pages: 10,
            outlines: vec![
                Bookmark {
                    title: "C1".into(),
                    page_number: 1,
                    level: 0,
                },
                Bookmark {
                    title: "C2".into(),
                    page_number: 5,
                    level: 0,
                },
            ],
        };
        let ranges = info.calculate_bookmark_ranges();
        assert_eq!(
            ranges,
            vec![
                ChunkMetadata {
                    start_page: 1,
                    end_page: 4,
                    bookmark_index: Some(1),
                    bookmark_title: Some("C1".into())
                },
                ChunkMetadata {
                    start_page: 5,
                    end_page: 10,
                    bookmark_index: Some(2),
                    bookmark_title: Some("C2".into())
                },
            ]
        );
    }

    #[test]
    fn test_calculate_bookmark_ranges_nested() {
        let info = PdfInfo {
            total_pages: 10,
            outlines: vec![
                Bookmark {
                    title: "C1".into(),
                    page_number: 1,
                    level: 0,
                },
                Bookmark {
                    title: "C1.1".into(),
                    page_number: 2,
                    level: 1,
                },
                Bookmark {
                    title: "C2".into(),
                    page_number: 5,
                    level: 0,
                },
            ],
        };
        let ranges = info.calculate_bookmark_ranges();
        assert_eq!(
            ranges,
            vec![
                ChunkMetadata {
                    start_page: 1,
                    end_page: 1,
                    bookmark_index: Some(1),
                    bookmark_title: Some("C1".into())
                },
                ChunkMetadata {
                    start_page: 2,
                    end_page: 4,
                    bookmark_index: Some(2),
                    bookmark_title: Some("C1.1".into())
                },
                ChunkMetadata {
                    start_page: 5,
                    end_page: 10,
                    bookmark_index: Some(3),
                    bookmark_title: Some("C2".into())
                },
            ]
        );
    }

    #[test]
    fn test_calculate_bookmark_ranges_unsorted() {
        let info = PdfInfo {
            total_pages: 10,
            outlines: vec![
                Bookmark {
                    title: "C2".into(),
                    page_number: 5,
                    level: 0,
                },
                Bookmark {
                    title: "C1".into(),
                    page_number: 1,
                    level: 0,
                },
            ],
        };
        let ranges = info.calculate_bookmark_ranges();
        assert_eq!(
            ranges,
            vec![
                ChunkMetadata {
                    start_page: 1,
                    end_page: 4,
                    bookmark_index: Some(2),
                    bookmark_title: Some("C1".into())
                },
                ChunkMetadata {
                    start_page: 5,
                    end_page: 10,
                    bookmark_index: Some(1),
                    bookmark_title: Some("C2".into())
                },
            ]
        );
    }

    #[test]
    fn test_calculate_hybrid_ranges_simple() {
        let pdf = PdfInfo {
            total_pages: 20,
            outlines: vec![
                Bookmark {
                    title: "Ch1".to_string(),
                    page_number: 1,
                    level: 0,
                },
                Bookmark {
                    title: "Ch2".to_string(),
                    page_number: 10,
                    level: 0,
                },
            ],
        };

        let ranges = pdf.calculate_hybrid_ranges(5);
        assert_eq!(
            ranges,
            vec![
                ChunkMetadata {
                    start_page: 1,
                    end_page: 5,
                    bookmark_index: Some(1),
                    bookmark_title: Some("Ch1".into())
                },
                ChunkMetadata {
                    start_page: 6,
                    end_page: 9,
                    bookmark_index: Some(1),
                    bookmark_title: Some("Ch1".into())
                },
                ChunkMetadata {
                    start_page: 10,
                    end_page: 14,
                    bookmark_index: Some(2),
                    bookmark_title: Some("Ch2".into())
                },
                ChunkMetadata {
                    start_page: 15,
                    end_page: 19,
                    bookmark_index: Some(2),
                    bookmark_title: Some("Ch2".into())
                },
                ChunkMetadata {
                    start_page: 20,
                    end_page: 20,
                    bookmark_index: Some(2),
                    bookmark_title: Some("Ch2".into())
                },
            ]
        );
    }

    #[test]
    fn test_calculate_hybrid_ranges_no_bookmarks() {
        let pdf = PdfInfo {
            total_pages: 10,
            outlines: vec![],
        };
        let ranges = pdf.calculate_hybrid_ranges(5);
        assert_eq!(
            ranges,
            vec![
                ChunkMetadata {
                    start_page: 1,
                    end_page: 5,
                    bookmark_index: None,
                    bookmark_title: None
                },
                ChunkMetadata {
                    start_page: 6,
                    end_page: 10,
                    bookmark_index: None,
                    bookmark_title: None
                },
            ]
        );
    }

    #[test]
    fn test_calculate_hybrid_ranges_overlap_and_boundary() {
        let pdf = PdfInfo {
            total_pages: 10,
            outlines: vec![
                Bookmark {
                    title: "Start".into(),
                    page_number: 1,
                    level: 0,
                },
                Bookmark {
                    title: "Mid".into(),
                    page_number: 5,
                    level: 0,
                },
                Bookmark {
                    title: "End".into(),
                    page_number: 10,
                    level: 0,
                },
            ],
        };

        let ranges = pdf.calculate_hybrid_ranges(100);
        assert_eq!(
            ranges,
            vec![
                ChunkMetadata {
                    start_page: 1,
                    end_page: 4,
                    bookmark_index: Some(1),
                    bookmark_title: Some("Start".into())
                },
                ChunkMetadata {
                    start_page: 5,
                    end_page: 9,
                    bookmark_index: Some(2),
                    bookmark_title: Some("Mid".into())
                },
                ChunkMetadata {
                    start_page: 10,
                    end_page: 10,
                    bookmark_index: Some(3),
                    bookmark_title: Some("End".into())
                },
            ]
        );
    }

    #[test]
    fn test_calculate_hybrid_ranges_duplicate_pages() {
        let pdf = PdfInfo {
            total_pages: 10,
            outlines: vec![
                Bookmark {
                    title: "A".into(),
                    page_number: 5,
                    level: 0,
                },
                Bookmark {
                    title: "B".into(),
                    page_number: 5,
                    level: 1,
                },
            ],
        };

        let ranges = pdf.calculate_hybrid_ranges(10);
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start_page, 1);
        assert_eq!(ranges[0].end_page, 4);
        assert_eq!(ranges[1].start_page, 5);
        assert_eq!(ranges[1].end_page, 10);
    }

    fn calculate_ranges_helper(total_pages: u32, pages_per_chunk: u32) -> Vec<(u32, u32)> {
        let info = PdfInfo {
            total_pages,
            outlines: vec![],
        };
        info.calculate_page_ranges(pages_per_chunk)
            .into_iter()
            .map(|m| (m.start_page, m.end_page))
            .collect()
    }
}
