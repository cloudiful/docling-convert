mod args;
mod logging;

use args::Args;
use logging::init_logging;

use bytes::Bytes;
use clap::Parser;
use cloudiful_docling_convert::{
    ConvertRequest, DocumentConverter, FileConvertRequest, InputDocument, InputKind,
    PdfConvertError, Result, build_convert_options, build_docling_client, supported_input_kind,
};
use log::{error, info};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let args = Args::parse();

    if let Err(e) = init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    info!(
        "Starting cloudiful-docling-convert v{}",
        env!("CARGO_PKG_VERSION")
    );

    log::debug!("Input paths: {:?}", args.input_files);
    log::debug!("Output directory: {}", args.output_dir.display());
    log::debug!("Output format: {}", args.format);
    log::debug!("Split input: {}", args.split_input);

    if let Err(e) = run_conversions(args).await {
        error!("Conversion failed: {}", e);

        match &e {
            PdfConvertError::EnvError { var_name, .. } => {
                eprintln!("\nError: Missing environment variable '{}'", var_name);
                eprintln!("Please set it using: export {}=<your_api_key>", var_name);
            }
            PdfConvertError::ValidationError { parameter, reason } => {
                eprintln!("\nError: Invalid argument '{}': {}", parameter, reason);
            }
            PdfConvertError::IoError { context, source } => {
                eprintln!("\nError: {}: {}", context, source);
            }
            _ => {
                eprintln!("\nError: {}", e);
            }
        }

        std::process::exit(1);
    }

    info!("Document conversion completed successfully");
}

async fn run_conversions(args: Args) -> Result<()> {
    let mut files_to_process = collect_input_files(&args)?;

    if files_to_process.is_empty() {
        info!("No supported files found to process.");
        return Ok(());
    }

    files_to_process.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    info!(
        "Found {} supported files to process",
        files_to_process.len()
    );
    let mut failed_files = Vec::new();

    for file_path in files_to_process {
        let output_path = DocumentConverter::calculate_output_path(
            &args.output_dir,
            file_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("output"),
            args.format,
        );

        if !args.overwrite && output_path.exists() {
            info!(
                "Skipping already converted file: {} (output exists: {})",
                file_path.display(),
                output_path.display()
            );
            continue;
        }

        info!("Processing: {}", file_path.display());
        let start_time = std::time::Instant::now();

        let file_args = args.clone();

        match convert_single_file(file_path.clone(), file_args).await {
            Ok(_) => {
                let duration = start_time.elapsed();
                info!(
                    "✓ Finished converting {} in {:.2}s",
                    file_path.display(),
                    duration.as_secs_f32()
                );
            }
            Err(e) => {
                error!("Failed to convert {}: {}", file_path.display(), e);
                failed_files.push((file_path, e.to_string()));
            }
        }
    }

    if !failed_files.is_empty() {
        let summary = failed_files
            .iter()
            .map(|(path, err)| format!("{} ({})", path.display(), err))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(PdfConvertError::operation_error(
            "converting input files",
            format!("{} file(s) failed: {}", failed_files.len(), summary),
        ));
    }

    Ok(())
}

fn collect_input_files(args: &Args) -> Result<Vec<std::path::PathBuf>> {
    if let Some(input_kind) = args.input_format {
        return collect_override_input_file(args, input_kind);
    }

    let mut files_to_process = Vec::new();

    for path in &args.input_files {
        if path.is_file() {
            if is_supported_input_file(path) {
                files_to_process.push(path.clone());
            } else {
                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                let reason = if InputKind::requires_explicit_override(file_name, None) {
                    format!(
                        "ambiguous input type for '{}'; use --input-format to choose the Docling source format",
                        path.display()
                    )
                } else {
                    format!("unsupported file type for '{}'", path.display())
                };
                return Err(PdfConvertError::validation_error("input_path", reason));
            }
            continue;
        }

        if path.is_dir() {
            let entries = std::fs::read_dir(path).map_err(|e| {
                PdfConvertError::io_error(format!("reading directory: {}", path.display()), e)
            })?;

            for entry in entries {
                let entry = entry.map_err(|e| {
                    PdfConvertError::io_error(
                        format!("reading directory entry in: {}", path.display()),
                        e,
                    )
                })?;
                let entry_path = entry.path();
                if is_supported_input_file(&entry_path) {
                    files_to_process.push(entry_path);
                }
            }
        }
    }

    Ok(files_to_process)
}

fn collect_override_input_file(
    args: &Args,
    input_kind: InputKind,
) -> Result<Vec<std::path::PathBuf>> {
    if args.input_files.len() != 1 {
        return Err(PdfConvertError::validation_error(
            "input_format",
            format!(
                "--input-format {} requires exactly one input file path",
                input_kind
            ),
        ));
    }

    let path = args
        .input_files
        .first()
        .expect("checked input file count above");

    if path.is_dir() {
        return Err(PdfConvertError::validation_error(
            "input_format",
            format!(
                "--input-format {} only supports a single file input, not directory scanning",
                input_kind
            ),
        ));
    }

    if !path.is_file() {
        return Err(PdfConvertError::validation_error(
            "input_path",
            format!("input file does not exist: {}", path.display()),
        ));
    }

    Ok(vec![path.clone()])
}

fn is_supported_input_file(path: &std::path::Path) -> bool {
    path.is_file() && supported_input_kind(path)
}

async fn convert_single_file(file_path: std::path::PathBuf, args: Args) -> Result<()> {
    let file_bytes = tokio::fs::read(&file_path).await.map_err(|e| {
        PdfConvertError::io_error(format!("reading input file: {}", file_path.display()), e)
    })?;

    let input = match args.input_format {
        Some(input_kind) => InputDocument::from_path_and_bytes_with_kind(
            &file_path,
            Bytes::from(file_bytes),
            input_kind,
        )?,
        None => InputDocument::from_path_and_bytes(&file_path, Bytes::from(file_bytes))?,
    };
    let input_kind = input.kind()?;
    let converter = DocumentConverter::new(create_docling_client(&args)?);
    converter
        .convert_to_file(FileConvertRequest {
            request: ConvertRequest {
                input,
                output_formats: vec![args.format],
                options: build_convert_options(input_kind, &args.behavior())?,
            },
            output_dir: args.output_dir.clone(),
            selected_output: args.format,
            overwrite: args.overwrite,
        })
        .await?;

    Ok(())
}

fn create_docling_client(args: &Args) -> Result<cloudiful_docling_convert::DoclingClient> {
    build_docling_client(args.runtime_config())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Args;
    use clap::Parser;

    #[test]
    fn input_format_rejects_directory_scan() {
        let args = Args::try_parse_from([
            "cloudiful-docling-convert",
            "--input-format",
            "xml_jats",
            ".",
        ])
        .unwrap();

        let err = collect_input_files(&args).unwrap_err();
        assert!(err.to_string().contains("directory scanning"));
    }

    #[test]
    fn input_format_rejects_multiple_paths() {
        let args = Args::try_parse_from([
            "cloudiful-docling-convert",
            "--input-format",
            "xml_jats",
            "a.xml",
            "b.xml",
        ])
        .unwrap();

        let err = collect_input_files(&args).unwrap_err();
        assert!(err.to_string().contains("exactly one input file path"));
    }

    #[test]
    fn ambiguous_xml_file_requires_input_format_override() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("paper.xml");
        std::fs::write(&path, "<article />").unwrap();

        let args =
            Args::try_parse_from(["cloudiful-docling-convert", path.to_str().unwrap()]).unwrap();

        let err = collect_input_files(&args).unwrap_err();
        assert!(err.to_string().contains("--input-format"));
    }
}
