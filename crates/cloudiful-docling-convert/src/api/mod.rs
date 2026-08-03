mod chunk;
pub mod docling;
mod result;
mod source;
mod transport;
mod vlm;
mod vlm_config;

#[cfg(test)]
mod docling_tests;

pub use docling::{DoclingClient, DoclingConfig, DoclingConvertRequest};
pub use result::{DoclingResult, DoclingTaskResult};
