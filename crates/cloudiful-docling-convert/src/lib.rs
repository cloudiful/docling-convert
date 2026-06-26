mod api;
mod conversion;
mod document;
mod error;
mod facade;
mod models;
mod pdf;
mod processor;

pub use api::{DoclingClient, DoclingConfig, DoclingConvertRequest};
pub use conversion::{
    ConversionBehavior, DoclingRuntimeConfig, build_convert_options, build_docling_client,
    build_pdf_options, count_input_chunks,
};
pub use document::{
    ConvertOptions, ConvertRequest, ConvertedChunk, ConvertedDocument, ConvertedDocumentMetadata,
    ConvertedFile, FileConvertRequest, GenericFileConvertOptions, InputDocument, InputKind,
    OutputFormat, PdfConvertOptions, TextConvertOptions, supported_input_kind,
};
pub use error::{PdfConvertError, Result};
pub use facade::{ConverterBuilder, PdfConvert};
pub use models::{Bookmark, ChunkMetadata, PdfInfo};
pub use processor::{DocumentConverter, build_pdf_chunk_plan};
