use clap::Parser;
use cloudiful_docling_convert::{
    ChunkerKind, ChunkingOptions, ConversionBehavior, DoclingRuntimeConfig, InputKind,
    OutputFormat, PipelineKind,
};
use std::path::PathBuf;
use std::time::Duration;

fn parse_output_format(value: &str) -> Result<OutputFormat, String> {
    value
        .parse::<OutputFormat>()
        .map_err(|error| error.to_string())
}

fn parse_input_format(value: &str) -> Result<InputKind, String> {
    value
        .parse::<InputKind>()
        .map_err(|error| error.to_string())
}

fn parse_chunker(value: &str) -> Result<ChunkerKind, String> {
    value
        .parse::<ChunkerKind>()
        .map_err(|error| error.to_string())
}

fn parse_pipeline(value: &str) -> Result<PipelineKind, String> {
    value
        .parse::<PipelineKind>()
        .map_err(|error| error.to_string())
}

fn parse_positive_u32(value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| format!("invalid integer value: {value}"))?;
    (parsed > 0)
        .then_some(parsed)
        .ok_or_else(|| "value must be 1 or greater".to_string())
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "cloudiful-docling-convert",
    version,
    about = "Convert document files through Docling Serve"
)]
pub struct Args {
    #[arg(
        value_name = "INPUT_PATHS",
        help = "Paths to supported files or directories (default: .)",
        default_values = ["."]
    )]
    pub input_files: Vec<PathBuf>,

    #[arg(
        short = 'o',
        long,
        value_name = "OUTPUT_DIR",
        help = "Output directory (default: .)",
        default_value = "."
    )]
    pub output_dir: PathBuf,

    #[arg(
        short = 'u',
        long,
        default_value = "http://127.0.0.1:5001/v1",
        value_name = "URL",
        help = "Docling Serve API base URL"
    )]
    pub docling_base_url: String,

    #[arg(
        short = 'f',
        long,
        value_parser = parse_output_format,
        default_value = "md",
        value_name = "FORMAT",
        help = "Output format: md, json, yaml, html, html_split_page, text, doctags, vtt, doclang, dclx, chunks"
    )]
    pub format: OutputFormat,

    #[arg(
        long,
        value_parser = parse_input_format,
        value_name = "FORMAT",
        help = "Explicit input format override for ambiguous or extensionless files"
    )]
    pub input_format: Option<InputKind>,

    #[arg(
        long,
        value_parser = parse_chunker,
        default_value = "none",
        value_name = "CHUNKER",
        help = "Native Docling chunker: none, hybrid, or hierarchical"
    )]
    pub chunker: ChunkerKind,

    #[arg(long, help = "Serialize tables as Markdown in native chunks")]
    pub use_markdown_tables: bool,

    #[arg(long, help = "Include Markdown image references in native chunks")]
    pub use_markdown_images: bool,

    #[arg(
        long,
        default_value = "![IMAGE]",
        help = "Image placeholder in native chunks"
    )]
    pub image_placeholder: String,

    #[arg(long, help = "Include raw text alongside contextualized chunk text")]
    pub include_raw_text: bool,

    #[arg(long, value_parser = parse_positive_u32, help = "Maximum tokens per hybrid chunk")]
    pub max_tokens: Option<u32>,

    #[arg(
        long,
        value_name = "MODEL",
        help = "Tokenizer model for hybrid chunking"
    )]
    pub tokenizer: Option<String>,

    #[arg(
        long,
        default_value_t = true,
        help = "Merge undersized hybrid peer chunks"
    )]
    pub merge_peers: bool,

    #[arg(long, value_parser = parse_pipeline, value_name = "PIPELINE")]
    pub pipeline: Option<PipelineKind>,

    #[arg(long, value_name = "URL", help = "OpenAI-compatible VLM API base URL")]
    pub openai_base_url: Option<String>,

    #[arg(long, value_name = "MODEL")]
    pub vlm_pipeline_model: Option<String>,

    #[arg(long, value_name = "MODEL")]
    pub picture_description_model: Option<String>,

    #[arg(long, value_name = "MODEL")]
    pub code_formula_model: Option<String>,

    #[arg(long, help = "Overwrite existing output files")]
    pub overwrite: bool,
}

impl Args {
    pub fn behavior(&self) -> ConversionBehavior {
        ConversionBehavior {
            chunker: self.chunker,
            chunking: ChunkingOptions {
                use_markdown_tables: self.use_markdown_tables,
                use_markdown_images: self.use_markdown_images,
                image_placeholder: self.image_placeholder.clone(),
                include_raw_text: self.include_raw_text,
                max_tokens: self.max_tokens,
                tokenizer: self.tokenizer.clone(),
                merge_peers: self.merge_peers,
            },
            pipeline: self.pipeline,
        }
    }

    pub fn runtime_config(&self) -> DoclingRuntimeConfig {
        DoclingRuntimeConfig {
            docling_base_url: self.docling_base_url.clone(),
            openai_base_url: self.openai_base_url.clone().unwrap_or_default(),
            vlm_pipeline_model: self.vlm_pipeline_model.clone().unwrap_or_default(),
            picture_description_model: self.picture_description_model.clone().unwrap_or_default(),
            code_formula_model: self.code_formula_model.clone().unwrap_or_default(),
            api_key: std::env::var("DOCLING_API_KEY").ok(),
            tenant_id: std::env::var("DOCLING_TENANT_ID").ok(),
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            request_timeout: std::env::var("DOCLING_HTTP_TIMEOUT_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs),
            task_timeout: std::env::var("DOCLING_TASK_TIMEOUT_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_native_chunker_and_pipeline() {
        let args = Args::try_parse_from([
            "cloudiful-docling-convert",
            "--format",
            "chunks",
            "--chunker",
            "hybrid",
            "--pipeline",
            "asr",
        ])
        .unwrap();

        assert_eq!(args.format, OutputFormat::Chunks);
        assert_eq!(args.chunker, ChunkerKind::Hybrid);
        assert_eq!(args.pipeline, Some(PipelineKind::Asr));
    }

    #[test]
    fn parses_input_format_override() {
        let args = Args::try_parse_from([
            "cloudiful-docling-convert",
            "--input-format",
            "xml_jats",
            "paper.xml",
        ])
        .unwrap();

        assert_eq!(args.input_format, Some(InputKind::XmlJats));
    }

    #[test]
    fn rejects_zero_max_tokens() {
        let error =
            Args::try_parse_from(["cloudiful-docling-convert", "--max-tokens", "0"]).unwrap_err();
        assert!(error.to_string().contains("1 or greater"));
    }
}
