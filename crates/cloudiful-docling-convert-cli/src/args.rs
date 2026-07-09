use clap::{ArgAction, Parser};
use cloudiful_docling_convert::{ConversionBehavior, DoclingRuntimeConfig, OutputFormat};
use std::path::PathBuf;

fn parse_positive_u32(value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| format!("invalid integer value: {}", value))?;
    if parsed == 0 {
        return Err("value must be 1 or greater".to_string());
    }
    Ok(parsed)
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("invalid integer value: {}", value))?;
    if parsed == 0 {
        return Err("value must be 1 or greater".to_string());
    }
    Ok(parsed)
}

fn parse_output_format(value: &str) -> Result<OutputFormat, String> {
    value
        .parse::<OutputFormat>()
        .map_err(|error| error.to_string())
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "cloudiful-docling-convert",
    version,
    about = "Convert document files through the cloudiful-docling-convert library and Docling API",
    long_about = "A CLI tool built on the cloudiful-docling-convert library. It converts PDF, Office, HTML, \
                  AsciiDoc, spreadsheet, image, EPUB, email, Markdown, and TXT inputs into structured Markdown, \
                  text, JSON, HTML, or DocTags output. PDF inputs keep \
                  split/bookmark-aware processing and parallel submission through Docling."
)]
pub struct Args {
    #[arg(
        value_name = "INPUT_PATHS",
        help = "Paths to supported files or directories to convert (default: .)",
        default_values = ["."]
    )]
    pub input_files: Vec<PathBuf>,

    #[arg(
        short = 'o',
        long,
        value_name = "OUTPUT_DIR",
        help = "Output directory for converted files (default: .)",
        default_value = "."
    )]
    pub output_dir: PathBuf,

    #[arg(
        short = 'n',
        long,
        default_value_t = 5,
        value_parser = parse_positive_u32,
        value_name = "PAGES",
        help = "Number of pages per split chunk (default: 5)"
    )]
    pub pages_per_file: u32,

    #[arg(
        short = 'u',
        long,
        default_value = "http://127.0.0.1:5001/v1",
        value_name = "URL",
        help = "Docling API base URL"
    )]
    pub docling_base_url: String,

    #[arg(
        short = 'f',
        long,
        value_parser = parse_output_format,
        default_value = "md",
        value_name = "FORMAT",
        help = "Output format: json, md, text, html, or doctags (default: md)"
    )]
    pub format: OutputFormat,

    #[arg(
        long,
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = true,
        help = "Split PDF input into chunks before processing (default: true)"
    )]
    pub split_input: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Split PDF input based on bookmarks/outlines (default: false)"
    )]
    pub split_by_bookmark: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Enable semantic chunking in output (default: false)"
    )]
    pub chunking: bool,

    #[arg(
        long,
        value_name = "URL",
        help = "OpenAI-compatible API base URL for optional VLM enrichment"
    )]
    pub openai_base_url: Option<String>,

    #[arg(long, value_name = "MODEL", help = "Optional VLM pipeline model")]
    pub vlm_pipeline_model: Option<String>,

    #[arg(
        long,
        value_name = "MODEL",
        help = "Optional VLM model for picture descriptions"
    )]
    pub picture_description_model: Option<String>,

    #[arg(
        long,
        value_name = "MODEL",
        help = "Optional VLM model for code and formula recognition"
    )]
    pub code_formula_model: Option<String>,

    #[arg(
        short = 'b',
        long,
        default_value_t = 2,
        value_parser = parse_positive_usize,
        value_name = "SIZE",
        help = "Number of tasks to submit in parallel (default: 2)"
    )]
    pub batch_size: usize,

    #[arg(
        long,
        default_value_t = false,
        help = "Overwrite existing output files (default: false)"
    )]
    pub overwrite: bool,
}

impl Args {
    pub fn behavior(&self) -> ConversionBehavior {
        ConversionBehavior {
            pages_per_file: self.pages_per_file,
            split_input: self.split_input,
            split_by_bookmark: self.split_by_bookmark,
            chunking: self.chunking,
            batch_size: self.batch_size,
        }
    }

    pub fn runtime_config(&self) -> DoclingRuntimeConfig {
        DoclingRuntimeConfig {
            docling_base_url: self.docling_base_url.clone(),
            openai_base_url: self.openai_base_url.clone().unwrap_or_default(),
            vlm_pipeline_model: self.vlm_pipeline_model.clone().unwrap_or_default(),
            picture_description_model: self.picture_description_model.clone().unwrap_or_default(),
            code_formula_model: self.code_formula_model.clone().unwrap_or_default(),
            api_key: std::env::var("OPENAI_API_KEY").ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typed_format_and_explicit_false_flags() {
        let args = Args::try_parse_from([
            "cloudiful-docling-convert",
            "--format",
            "html",
            "--split-input=false",
        ])
        .expect("CLI arguments should parse");

        assert_eq!(args.format, OutputFormat::Html);
        assert!(!args.split_input);
    }

    #[test]
    fn rejects_zero_for_positive_numeric_arguments() {
        let pages_err =
            Args::try_parse_from(["cloudiful-docling-convert", "--pages-per-file", "0"])
                .unwrap_err();
        assert!(pages_err.to_string().contains("1 or greater"));

        let batch_err =
            Args::try_parse_from(["cloudiful-docling-convert", "--batch-size", "0"]).unwrap_err();
        assert!(batch_err.to_string().contains("1 or greater"));
    }

    #[test]
    fn vlm_flags_are_optional() {
        let args = Args::try_parse_from(["cloudiful-docling-convert"]).unwrap();
        let runtime = args.runtime_config();

        assert_eq!(runtime.openai_base_url, "");
        assert_eq!(runtime.vlm_pipeline_model, "");
        assert_eq!(runtime.picture_description_model, "");
        assert_eq!(runtime.code_formula_model, "");
    }
}
