use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::Mutex;

use super::DocumentConverter;
use crate::api::{DoclingClient, DoclingConfig};
use crate::document::{
    ConvertOptions, ConvertRequest, FileConvertRequest, InputDocument, InputKind, OutputFormat,
    TextConvertOptions,
};

fn test_converter() -> DocumentConverter {
    let client = DoclingClient::new(DoclingConfig {
        base_url: "http://localhost:5001/v1".to_string(),
        openai_base_url: "http://localhost:1234/v1".to_string(),
        vlm_pipeline_model: "vlm".to_string(),
        picture_description_model: "pic".to_string(),
        code_formula_model: "code".to_string(),
        api_key: None,
    })
    .unwrap();

    DocumentConverter::new(client)
}

#[tokio::test]
async fn txt_input_uses_local_conversion() {
    let converter = test_converter();
    let request = ConvertRequest {
        input: InputDocument::new("notes.txt", "text/plain", Bytes::from("a\r\nb")),
        output_formats: vec![
            OutputFormat::Md,
            OutputFormat::Text,
            OutputFormat::Json,
            OutputFormat::Html,
            OutputFormat::Doctags,
        ],
        options: ConvertOptions::Text(TextConvertOptions::default()),
    };

    let document = converter.convert(request).await.unwrap();
    assert_eq!(document.metadata.input_kind, InputKind::Text);
    assert_eq!(document.text.as_deref(), Some("a\nb"));
    assert_eq!(document.markdown.as_deref(), Some("a\nb"));
    assert!(
        document
            .html
            .as_deref()
            .is_some_and(|html| html.contains("<pre>a\nb</pre>"))
    );
    assert_eq!(document.doctags.as_deref(), Some("a\nb"));
    assert!(document.json.is_some());
}

#[tokio::test]
async fn convert_to_file_with_progress_reports_text_completion() {
    let converter = test_converter();
    let temp_dir = tempfile::tempdir().unwrap();
    let progress = Arc::new(Mutex::new(Vec::new()));
    let progress_log = Arc::clone(&progress);

    let result = converter
        .convert_to_file_with_progress(
            FileConvertRequest {
                request: ConvertRequest {
                    input: InputDocument::new("notes.txt", "text/plain", Bytes::from("hello")),
                    output_formats: vec![OutputFormat::Text],
                    options: ConvertOptions::Text(TextConvertOptions::default()),
                },
                output_dir: temp_dir.path().to_path_buf(),
                selected_output: OutputFormat::Text,
                overwrite: true,
            },
            move |completed, total| {
                let progress_log = Arc::clone(&progress_log);
                async move {
                    progress_log.lock().await.push((completed, total));
                }
            },
        )
        .await
        .unwrap();

    assert_eq!(*progress.lock().await, vec![(1, 1)]);
    assert_eq!(result.output_paths.len(), 1);
}

#[tokio::test]
async fn convert_to_file_writes_html_and_doctags_for_text_input() {
    let converter = test_converter();
    let temp_dir = tempfile::tempdir().unwrap();

    let html_result = converter
        .convert_to_file(FileConvertRequest {
            request: ConvertRequest {
                input: InputDocument::new("notes.txt", "text/plain", Bytes::from("hello")),
                output_formats: vec![OutputFormat::Html],
                options: ConvertOptions::Text(TextConvertOptions::default()),
            },
            output_dir: temp_dir.path().to_path_buf(),
            selected_output: OutputFormat::Html,
            overwrite: true,
        })
        .await
        .unwrap();
    let html = tokio::fs::read_to_string(&html_result.output_paths[0])
        .await
        .unwrap();
    assert!(html.contains("<pre>hello</pre>"));

    let doctags_result = converter
        .convert_to_file(FileConvertRequest {
            request: ConvertRequest {
                input: InputDocument::new("notes.txt", "text/plain", Bytes::from("hello")),
                output_formats: vec![OutputFormat::Doctags],
                options: ConvertOptions::Text(TextConvertOptions::default()),
            },
            output_dir: temp_dir.path().to_path_buf(),
            selected_output: OutputFormat::Doctags,
            overwrite: true,
        })
        .await
        .unwrap();
    let doctags = tokio::fs::read_to_string(&doctags_result.output_paths[0])
        .await
        .unwrap();
    assert_eq!(doctags, "hello");
}

#[test]
fn file_output_path_uses_selected_format() {
    let path = DocumentConverter::calculate_output_path(
        Path::new("/tmp/out"),
        "sample.docx",
        OutputFormat::Text,
    );
    assert_eq!(path, PathBuf::from("/tmp/out/sample.text"));
}

#[test]
fn file_output_path_uses_doctags_extension() {
    let path = DocumentConverter::calculate_output_path(
        Path::new("/tmp/out"),
        "sample.docx",
        OutputFormat::Doctags,
    );
    assert_eq!(path, PathBuf::from("/tmp/out/sample.doctags"));
}
