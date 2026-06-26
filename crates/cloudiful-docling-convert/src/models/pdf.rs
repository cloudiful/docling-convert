use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfInfo {
    pub total_pages: u32,
    pub outlines: Vec<Bookmark>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub page_number: u32,
    pub level: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkMetadata {
    pub start_page: u32,
    pub end_page: u32,
    pub bookmark_index: Option<usize>,
    pub bookmark_title: Option<String>,
}
