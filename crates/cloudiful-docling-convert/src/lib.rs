mod api;
mod conversion;
mod document;
mod error;
mod facade;
mod models;
mod processor;

pub use api::{
    DoclingClient, DoclingConfig, DoclingConvertRequest, DoclingResult, DoclingTaskResult,
};
pub use conversion::{
    ConversionBehavior, DoclingRuntimeConfig, build_convert_options, build_docling_client,
};
pub use document::{
    ChunkerKind, ChunkingOptions, ConvertOptions, ConvertRequest, ConvertedDocument,
    ConvertedDocumentMetadata, ConvertedFile, FileConvertRequest, GenericFileConvertOptions,
    InputDocument, InputKind, OutputFormat, PdfConvertOptions, PipelineKind, RemoteConvertOptions,
    TextConvertOptions, supported_input_kind,
};
pub use error::{PdfConvertError, Result};
pub use facade::{ConverterBuilder, DoclingTaskHandle, PdfConvert};
pub use models::{
    ChunkDocumentResponse, ConversionStatus, ConvertDocumentResponse, DoclingChunk,
    DoclingErrorItem, DocumentResultItem, ExportDocumentResponse, PublicFailureInfo,
    TaskFailureResult, TaskPostResponse, TaskProcessingMeta, TaskStatusResponse,
};
pub use processor::DocumentConverter;
