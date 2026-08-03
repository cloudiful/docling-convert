use std::sync::Arc;

use bytes::Bytes;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{Duration, timeout};

use super::*;
use crate::document::{ChunkerKind, ChunkingOptions, InputDocument, InputKind, OutputFormat};
use crate::models::ConversionStatus;

struct MockResponse {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

impl MockResponse {
    fn json(body: Value) -> Self {
        Self {
            status: 200,
            content_type: "application/json".to_string(),
            body: serde_json::to_vec(&body).unwrap(),
        }
    }

    fn zip(body: &[u8]) -> Self {
        Self {
            status: 200,
            content_type: "application/zip".to_string(),
            body: body.to_vec(),
        }
    }

    fn error(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: content_type.to_string(),
            body: body.into(),
        }
    }
}

struct MockDoclingServer {
    base_url: String,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    task: JoinHandle<()>,
}

impl MockDoclingServer {
    async fn start(responses: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured_requests = Arc::clone(&requests);

        let task = tokio::spawn(async move {
            for response in responses {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let request = timeout(Duration::from_secs(5), read_request(&mut stream))
                    .await
                    .unwrap_or_else(|_| Ok(Vec::new()))
                    .unwrap_or_default();
                captured_requests.lock().await.push(request);

                let reason = match response.status {
                    200 => "OK",
                    422 => "Unprocessable Entity",
                    _ => "Error",
                };
                let headers = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.status,
                    reason,
                    response.content_type,
                    response.body.len()
                );
                let _ = stream.write_all(headers.as_bytes()).await;
                let _ = stream.write_all(&response.body).await;
            }
        });

        Self {
            base_url: format!("http://{address}/v1"),
            requests,
            task,
        }
    }

    async fn requests(&self) -> Vec<Vec<u8>> {
        self.requests.lock().await.clone()
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
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
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

fn authenticated_client(base_url: &str) -> DoclingClient {
    let mut config = DoclingConfig::without_vlm(base_url);
    config.api_key = Some("secret-key".to_string());
    config.tenant_id = Some("tenant-a".to_string());
    DoclingClient::new(config).unwrap()
}

fn input() -> InputDocument {
    InputDocument::new("notes.md", "text/markdown", Bytes::from_static(b"# hello"))
}

fn request() -> DoclingConvertRequest {
    DoclingConvertRequest::for_outputs(vec![OutputFormat::Md])
}

fn multipart_body(request: &[u8]) -> String {
    String::from_utf8_lossy(request)
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default()
}

#[tokio::test]
async fn error_responses_preserve_fastapi_details() {
    let body = r#"{"detail":[{"loc":["body","files"],"msg":"field required"}]}"#;
    let server =
        MockDoclingServer::start(vec![MockResponse::error(422, "application/json", body)]).await;

    let error = client(&server.base_url)
        .convert_file(&input(), &request())
        .await
        .expect_err("conversion should fail");
    let message = error.to_string();
    assert!(message.contains("HTTP 422"));
    assert!(message.contains("field required"));
    assert!(message.contains(body));
}

#[tokio::test]
async fn source_request_uses_auth_headers_and_target_object() {
    let server = MockDoclingServer::start(vec![MockResponse::json(json!({
        "document": "ok"
    }))])
    .await;

    let result = authenticated_client(&server.base_url)
        .convert_source("https://example.com/report.pdf", InputKind::Pdf, &request())
        .await
        .unwrap()
        .into_json()
        .unwrap();
    assert_eq!(result["document"], "ok");

    let request = String::from_utf8_lossy(&server.requests().await[0]).to_ascii_lowercase();
    assert!(request.starts_with("post /v1/convert/source http/1.1"));
    assert!(request.contains("x-api-key: secret-key"));
    assert!(request.contains("x-tenant-id: tenant-a"));
    let body = request.split_once("\r\n\r\n").unwrap().1;
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["sources"][0]["kind"], "http");
    assert_eq!(body["sources"][0]["url"], "https://example.com/report.pdf");
    assert_eq!(body["target"]["kind"], "inbody");
    assert_eq!(body["options"]["from_formats"][0], "pdf");
}

#[tokio::test]
async fn hybrid_chunk_request_uses_native_multipart_fields() {
    let server = MockDoclingServer::start(vec![MockResponse::json(json!({
        "chunks": [],
        "documents": [],
        "processing_time": 0.01
    }))])
    .await;
    let request = DoclingConvertRequest {
        output_formats: vec![OutputFormat::Chunks],
        page_range: None,
        chunker: ChunkerKind::Hybrid,
        chunking: ChunkingOptions {
            use_markdown_tables: true,
            use_markdown_images: true,
            image_placeholder: "[IMAGE]".to_string(),
            include_raw_text: true,
            max_tokens: Some(128),
            tokenizer: Some("test-tokenizer".to_string()),
            merge_peers: false,
        },
        pipeline: None,
    };

    authenticated_client(&server.base_url)
        .convert_file(&input(), &request)
        .await
        .unwrap();

    let captured = server.requests().await;
    let request = String::from_utf8_lossy(&captured[0]);
    let body = multipart_body(&captured[0]);
    assert!(request.starts_with("POST /v1/chunk/hybrid/file HTTP/1.1"));
    for field in [
        "convert_from_formats",
        "convert_to_formats",
        "chunking_use_markdown_tables",
        "chunking_use_markdown_images",
        "chunking_image_placeholder",
        "chunking_include_raw_text",
        "chunking_max_tokens",
        "chunking_tokenizer",
        "chunking_merge_peers",
    ] {
        assert!(body.contains(field), "missing multipart field {field}");
    }
    assert!(!body.contains("include_chunking"));
}

#[tokio::test]
async fn partial_success_polls_with_wait_and_reads_result_once() {
    let server = MockDoclingServer::start(vec![
        MockResponse::json(json!({"task_id": "task-1"})),
        MockResponse::json(json!({
            "task_id": "task-1",
            "task_type": "convert",
            "task_status": "partial_success",
            "task_meta": {
                "num_docs": 2,
                "num_processed": 2,
                "num_succeeded": 1,
                "num_partially_succeeded": 1,
                "num_failed": 0
            },
            "error_message": "one document had errors"
        })),
        MockResponse::json(json!({
            "document": {"filename": "notes.md", "md_content": "# hello"},
            "status": "partial_success",
            "processing_time": 0.1,
            "errors": []
        })),
    ])
    .await;

    let client = client(&server.base_url);
    let task_id = client
        .submit_file_async(&input(), &request())
        .await
        .unwrap();
    let result = client.wait_for_result(&task_id).await.unwrap();
    assert_eq!(result.status, ConversionStatus::PartialSuccess);
    assert_eq!(result.errors, vec!["one document had errors"]);
    assert!(matches!(result.result, DoclingResult::Convert(_)));

    let requests = server.requests().await;
    assert_eq!(requests.len(), 3);
    let poll = String::from_utf8_lossy(&requests[1]);
    assert!(poll.starts_with("GET /v1/status/poll/task-1?wait=30 HTTP/1.1"));
    assert!(String::from_utf8_lossy(&requests[2]).starts_with("GET /v1/result/task-1 HTTP/1.1"));
}

#[tokio::test]
async fn failure_status_does_not_require_an_unavailable_result() {
    let server = MockDoclingServer::start(vec![
        MockResponse::json(json!({"task_id": "failed-task"})),
        MockResponse::json(json!({
            "task_id": "failed-task",
            "task_type": "convert",
            "task_status": "failure",
            "error_message": "Internal processing error"
        })),
    ])
    .await;

    let client = client(&server.base_url);
    let task_id = client
        .submit_file_async(&input(), &request())
        .await
        .unwrap();
    let error = client
        .wait_for_result(&task_id)
        .await
        .expect_err("failure status should return an error");

    assert!(error.to_string().contains("Internal processing error"));
    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);
    assert!(
        String::from_utf8_lossy(&requests[1])
            .starts_with("GET /v1/status/poll/failed-task?wait=30 HTTP/1.1")
    );
}

#[tokio::test]
async fn zip_result_is_read_as_bytes() {
    let server = MockDoclingServer::start(vec![MockResponse::zip(b"PK\x03\x04zip")]).await;
    let result = client(&server.base_url)
        .get_task_result("zip-task")
        .await
        .unwrap();
    assert!(
        matches!(result, DoclingResult::Zip(bytes) if bytes == Bytes::from_static(b"PK\x03\x04zip"))
    );
}

#[test]
fn build_form_uses_input_media_type_and_format() {
    let client =
        DoclingClient::new(DoclingConfig::without_vlm("http://localhost:5001/v1")).unwrap();
    let input = InputDocument::new("notes.md", "text/markdown", Bytes::from_static(b"# hello"));
    let request = DoclingConvertRequest::for_outputs(vec![OutputFormat::Md, OutputFormat::Text]);

    let form = client.build_form(&input, &request).unwrap();
    let debug = format!("{form:?}");
    assert!(debug.contains("text/markdown"));
    assert!(debug.contains("notes.md"));
    assert!(debug.contains("to_formats"));
    assert!(debug.contains("target_type"));
}

#[test]
fn build_form_skips_page_range_for_generic_requests() {
    let client =
        DoclingClient::new(DoclingConfig::without_vlm("http://localhost:5001/v1")).unwrap();
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
