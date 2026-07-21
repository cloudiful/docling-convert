use std::sync::Arc;

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{Duration, timeout};

use super::*;
use crate::document::InputKind;

struct MockDoclingServer {
    base_url: String,
    request: Arc<Mutex<Option<Vec<u8>>>>,
    task: JoinHandle<()>,
}

impl MockDoclingServer {
    async fn start(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let request = Arc::new(Mutex::new(None));
        let captured_request = Arc::clone(&request);
        let content_type = content_type.to_string();
        let body = body.into();

        let task = tokio::spawn(async move {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let request = timeout(Duration::from_secs(5), read_request(&mut stream))
                .await
                .unwrap_or_else(|_| Ok(Vec::new()))
                .unwrap_or_default();
            *captured_request.lock().await = Some(request);

            let reason = match status {
                200 => "OK",
                422 => "Unprocessable Entity",
                _ => "Error",
            };
            let headers = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(headers.as_bytes()).await;
            let _ = stream.write_all(&body).await;
        });

        Self {
            base_url: format!("http://{address}/v1"),
            request,
            task,
        }
    }

    async fn request(&self) -> Vec<u8> {
        self.request
            .lock()
            .await
            .clone()
            .expect("mock server did not capture a request")
    }
}

impl Drop for MockDoclingServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn read_request(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        let count = stream.read(&mut buffer).await?;
        if count == 0 {
            return Ok(request);
        }
        request.extend_from_slice(&buffer[..count]);

        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let header_end = header_end + 4;
        let content_length = String::from_utf8_lossy(&request[..header_end])
            .lines()
            .find_map(|line| {
                line.strip_prefix("Content-Length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or_default();

        while request.len() < header_end + content_length {
            let count = stream.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
        }
        return Ok(request);
    }
}

fn client(base_url: &str) -> DoclingClient {
    DoclingClient::new(DoclingConfig::without_vlm(base_url)).unwrap()
}

fn input() -> InputDocument {
    InputDocument::new("notes.md", "text/markdown", Bytes::from_static(b"# hello"))
}

fn request() -> DoclingConvertRequest {
    DoclingConvertRequest::for_outputs(vec![OutputFormat::Md])
}

fn assert_multipart_request(request: &[u8], path: &str) {
    let request = String::from_utf8_lossy(request);
    assert!(request.starts_with(&format!("POST {path} HTTP/1.1")));
    assert!(request.contains("name=\"target_type\""));
    assert!(request.contains("inbody"));
}

#[tokio::test]
async fn sync_conversion_preserves_json_error_body_and_target_type() {
    let body = r#"{"detail":[{"loc":["body","files"],"msg":"field required"}]}"#;
    let server = MockDoclingServer::start(422, "application/json", body).await;

    let error = client(&server.base_url)
        .convert_file(&input(), &request())
        .await
        .expect_err("sync conversion should fail");
    let message = error.to_string();
    assert!(message.contains("HTTP 422"));
    assert!(message.contains("Docling file conversion"));
    assert!(message.contains("field required"));
    assert!(message.contains(body));
    assert_multipart_request(&server.request().await, "/v1/convert/file");
}

#[tokio::test]
async fn async_submission_preserves_json_error_body_and_target_type() {
    let body = r#"{"detail":[{"msg":"unsupported target"}]}"#;
    let server = MockDoclingServer::start(422, "application/json", body).await;

    let error = client(&server.base_url)
        .submit_file_async(&input(), &request())
        .await
        .expect_err("async submission should fail");
    let message = error.to_string();
    assert!(message.contains("HTTP 422"));
    assert!(message.contains("Docling async submission"));
    assert!(message.contains("unsupported target"));
    assert!(message.contains(body));
    assert_multipart_request(&server.request().await, "/v1/convert/file/async");
}

#[tokio::test]
async fn sync_conversion_preserves_complete_plain_text_error_body() {
    let body = format!("docling rejected request: {}", "x".repeat(256));
    let server = MockDoclingServer::start(422, "text/plain", body.clone()).await;

    let error = client(&server.base_url)
        .convert_file(&input(), &request())
        .await
        .expect_err("sync conversion should fail");
    let message = error.to_string();
    assert!(message.contains("HTTP 422"));
    assert!(message.contains(&body));
}

#[tokio::test]
async fn async_submission_preserves_plain_text_error_body() {
    let body = "docling rejected async submission";
    let server = MockDoclingServer::start(422, "text/plain", body).await;

    let error = client(&server.base_url)
        .submit_file_async(&input(), &request())
        .await
        .expect_err("async submission should fail");
    let message = error.to_string();
    assert!(message.contains("HTTP 422"));
    assert!(message.contains("Docling async submission"));
    assert!(message.contains(body));
}

#[tokio::test]
async fn sync_conversion_still_parses_success_response() {
    let body = r#"{"document":"ok"}"#;
    let server = MockDoclingServer::start(200, "application/json", body).await;

    let result = client(&server.base_url)
        .convert_file(&input(), &request())
        .await
        .expect("sync conversion should succeed");
    assert_eq!(result["document"], "ok");
    assert_multipart_request(&server.request().await, "/v1/convert/file");
}

#[tokio::test]
async fn async_submission_still_parses_success_response() {
    let server = MockDoclingServer::start(200, "application/json", r#"{"task_id":"task-1"}"#).await;

    let task_id = client(&server.base_url)
        .submit_file_async(&input(), &request())
        .await
        .expect("async submission should succeed");
    assert_eq!(task_id, "task-1");
    assert_multipart_request(&server.request().await, "/v1/convert/file/async");
}

#[test]
fn build_form_uses_input_media_type_and_format() {
    let client = DoclingClient::new(DoclingConfig {
        base_url: "http://localhost:5001/v1".to_string(),
        openai_base_url: "http://localhost:1234/v1".to_string(),
        vlm_pipeline_model: "vlm".to_string(),
        picture_description_model: "pic".to_string(),
        code_formula_model: "code".to_string(),
        api_key: Some("secret".to_string()),
    })
    .unwrap();

    let input = InputDocument::new("notes.md", "text/markdown", Bytes::from_static(b"# hello"));
    let request = DoclingConvertRequest {
        output_formats: vec![OutputFormat::Md, OutputFormat::Text],
        page_range: None,
        chunking: false,
    };

    let form = client.build_form(&input, &request).unwrap();
    let debug = format!("{form:?}");
    assert!(debug.contains("text/markdown"));
    assert!(debug.contains("notes.md"));
    assert!(debug.contains("to_formats"));
    assert!(debug.contains("target_type"));
}

#[test]
fn build_form_skips_page_range_for_generic_requests() {
    let client = DoclingClient::new(DoclingConfig {
        base_url: "http://localhost:5001/v1".to_string(),
        openai_base_url: "http://localhost:1234/v1".to_string(),
        vlm_pipeline_model: "vlm".to_string(),
        picture_description_model: "pic".to_string(),
        code_formula_model: "code".to_string(),
        api_key: Some("secret".to_string()),
    })
    .unwrap();

    let input = InputDocument::new(
        "doc.docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Bytes::from_static(b"PK"),
    );
    let request = DoclingConvertRequest::for_outputs(vec![OutputFormat::Md]);

    let form = client.build_form(&input, &request).unwrap();
    let debug = format!("{form:?}");
    assert!(!debug.contains("page_range"));
    assert!(debug.contains("from_formats"));
}

#[test]
fn build_form_uses_generic_first_wave_formats() {
    let client =
        DoclingClient::new(DoclingConfig::without_vlm("http://localhost:5001/v1")).unwrap();

    let input = InputDocument::new("sheet.csv", "text/csv", Bytes::from_static(b"a,b"));
    let request = DoclingConvertRequest::for_outputs(vec![OutputFormat::Html]);

    let form = client.build_form(&input, &request).unwrap();
    let debug = format!("{form:?}");
    assert!(debug.contains("text/csv"));
    assert!(debug.contains("sheet.csv"));
    assert!(debug.contains("csv"));
    assert!(debug.contains("to_formats"));
}

#[test]
fn build_form_supports_override_only_second_wave_formats() {
    let client =
        DoclingClient::new(DoclingConfig::without_vlm("http://localhost:5001/v1")).unwrap();

    let input = InputDocument::new(
        "filing.xml",
        "application/xml",
        Bytes::from_static(b"<article />"),
    )
    .with_input_kind(InputKind::XmlJats);
    let request = DoclingConvertRequest::for_outputs(vec![OutputFormat::Md]);

    let form = client.build_form(&input, &request).unwrap();
    let debug = format!("{form:?}");
    assert_eq!(input.kind().unwrap(), InputKind::XmlJats);
    assert!(debug.contains("application/xml"));
    assert!(debug.contains("from_formats"));
}

#[test]
fn build_form_skips_vlm_fields_when_runtime_config_is_missing() {
    let client =
        DoclingClient::new(DoclingConfig::without_vlm("http://localhost:5001/v1")).unwrap();

    let input = InputDocument::new("notes.md", "text/markdown", Bytes::from_static(b"# hello"));
    let request = DoclingConvertRequest::for_outputs(vec![OutputFormat::Md]);

    let form = client.build_form(&input, &request).unwrap();
    let debug = format!("{form:?}");
    assert!(!debug.contains("vlm_pipeline_custom_config"));
    assert!(!debug.contains("picture_description_custom_config"));
    assert!(!debug.contains("code_formula_custom_config"));
    assert!(!debug.contains("do_code_enrichment"));
    assert!(!debug.contains("do_formula_enrichment"));
    assert!(!debug.contains("do_picture_description"));
}
