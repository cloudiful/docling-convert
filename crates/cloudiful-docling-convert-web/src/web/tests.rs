use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    response::Response,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{Duration, timeout};
use tower::ServiceExt;

use super::conversion::process_url_conversion;
use super::handlers::create_router;
use super::state::{AppState, TaskConfig, TaskStatus, task_work_dir};
use super::support::sanitize_filename;

async fn response_json(response: Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn create_test_state() -> AppState {
    AppState::new(
        "http://127.0.0.1:1/v1".to_string(),
        "http://localhost:8080/v1".to_string(),
        "gpt-4o".to_string(),
        "gpt-4o-mini".to_string(),
        "gpt-4o-mini".to_string(),
    )
}

struct MockDoclingSourceServer {
    base_url: String,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    task: JoinHandle<()>,
}

impl MockDoclingSourceServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured_requests = Arc::clone(&requests);
        let responses = [
            json!({"task_id": "url-task"}),
            json!({
                "task_id": "url-task",
                "task_type": "convert",
                "task_status": "success",
                "task_meta": {"num_docs": 1, "num_processed": 1}
            }),
            json!({
                "document": {"filename": "notes.md", "text_content": "hello"},
                "status": "success",
                "processing_time": 0.01,
                "errors": []
            }),
        ];

        let task = tokio::spawn(async move {
            for response in responses {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let request = timeout(Duration::from_secs(5), read_http_request(&mut stream))
                    .await
                    .unwrap_or_else(|_| Ok(Vec::new()))
                    .unwrap_or_default();
                captured_requests.lock().await.push(request);
                let body = serde_json::to_vec(&response).unwrap();
                let headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(headers.as_bytes()).await;
                let _ = stream.write_all(&body).await;
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

impl Drop for MockDoclingSourceServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn read_http_request(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer).await?;
        if count == 0 {
            return Ok(request);
        }
        request.extend_from_slice(&buffer[..count]);
        if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
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
}

fn create_multipart_request(
    file_name: &str,
    content_type: &str,
    file_content: &[u8],
    config_fields: &[(&str, &str)],
) -> Request<Body> {
    let boundary = "----WebKitFormBoundaryTest";

    let mut body = format!(
        "--{boundary}\r\n\
        Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n\
        Content-Type: {}\r\n\r\n",
        file_name, content_type
    )
    .into_bytes();
    body.extend_from_slice(file_content);
    body.extend_from_slice(b"\r\n");

    for (key, value) in config_fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\n\
            Content-Disposition: form-data; name=\"{}\"\r\n\r\n\
            {}\r\n",
                key, value
            )
            .as_bytes(),
        );
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    Request::builder()
        .uri("/api/upload")
        .method("POST")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .header("Content-Length", body.len())
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn test_upload_valid_pdf() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let pdf_content = b"%PDF-1.4\n1 0 obj\nendobj\ntrailer\nstartxref\n0\n%%EOF";
    let request = create_multipart_request("test.pdf", "application/pdf", pdf_content, &[]);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();
    assert_eq!(json["filename"], "test.pdf");
    assert_eq!(json["message"], "File uploaded successfully");
    assert!(!json["task_id"].as_str().unwrap_or_default().is_empty());
    assert_eq!(task.total_chunks, 0);
}

#[tokio::test]
async fn test_upload_accepts_markdown() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let request = create_multipart_request("notes.md", "text/markdown", b"# hello", &[]);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();
    assert_eq!(task.filename, "notes.md");
    assert_eq!(task.total_chunks, 0);
}

#[tokio::test]
async fn test_upload_does_not_preparse_pdf_chunks() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let pdf_content = include_bytes!("../../../../test/full.pdf");
    let request = create_multipart_request("full.pdf", "application/pdf", pdf_content, &[]);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();
    assert_eq!(task.total_chunks, 0);
}

#[tokio::test]
async fn test_upload_accepts_csv() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let csv_content = b"name,value\nfoo,bar";
    let request = create_multipart_request("test.csv", "text/csv", csv_content, &[]);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();
    assert_eq!(task.filename, "test.csv");
}

#[tokio::test]
async fn test_upload_rejects_ambiguous_xml_without_override() {
    let state = create_test_state();
    let app: Router = create_router(state);

    let request = create_multipart_request("paper.xml", "application/xml", b"<article />", &[]);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_upload_accepts_xml_with_override() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let request = create_multipart_request(
        "paper.xml",
        "application/xml",
        b"<article />",
        &[("input_format", "xml_jats")],
    );
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();
    assert_eq!(task.config.input_format.as_deref(), Some("xml_jats"));
    assert_eq!(task.filename, "paper.xml");
}

#[tokio::test]
async fn test_upload_unsupported_file_type() {
    let state = create_test_state();
    let app: Router = create_router(state);

    let exe_content = b"MZ";
    let request =
        create_multipart_request("test.exe", "application/octet-stream", exe_content, &[]);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn test_health_check() {
    let state = create_test_state();
    let app: Router = create_router(state);

    let request = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_upload_applies_custom_task_config() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let pdf_content = b"%PDF-1.4\n1 0 obj\nendobj\ntrailer\nstartxref\n0\n%%EOF";
    let request = create_multipart_request(
        "configured.pdf",
        "application/pdf",
        pdf_content,
        &[
            ("format", "json"),
            ("chunking", "true"),
            ("pipeline", "standard"),
            ("include_raw_text", "true"),
        ],
    );
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();

    assert_eq!(task.config.format, "json");
    assert_eq!(
        task.config.chunker,
        cloudiful_docling_convert::ChunkerKind::Hybrid
    );
    assert!(task.config.chunking_options.include_raw_text);
    assert_eq!(
        task.config.pipeline,
        Some(cloudiful_docling_convert::PipelineKind::Standard)
    );
}

#[tokio::test]
async fn test_upload_persists_picture_description_preset() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let pdf_content = b"%PDF-1.4\n1 0 obj\nendobj\ntrailer\nstartxref\n0\n%%EOF";
    let request = create_multipart_request(
        "preset.pdf",
        "application/pdf",
        pdf_content,
        &[("picture_description_preset", "smolvlm")],
    );
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();
    assert_eq!(
        task.config.picture_description_preset.as_deref(),
        Some("smolvlm")
    );
}

#[tokio::test]
async fn test_upload_blank_picture_description_preset_keeps_default_unset() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let pdf_content = b"%PDF-1.4\n1 0 obj\nendobj\ntrailer\nstartxref\n0\n%%EOF";
    let request = create_multipart_request(
        "preset-blank.pdf",
        "application/pdf",
        pdf_content,
        &[("picture_description_preset", "   ")],
    );
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();
    assert!(task.config.picture_description_preset.is_none());
}

#[tokio::test]
async fn test_submit_url_persists_picture_description_preset() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let request = Request::builder()
        .uri("/api/convert/url")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"url":"https://example.com/notes.pdf","config":{"picture_description_preset":"granite_vision"}}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();
    assert_eq!(
        task.config.picture_description_preset.as_deref(),
        Some("granite_vision")
    );
}

#[tokio::test]
async fn test_list_tasks() {
    let state = create_test_state();
    state
        .create_task("first.pdf".to_string(), TaskConfig::default(), 1)
        .await;
    let app: Router = create_router(state);

    let request = Request::builder()
        .uri("/api/tasks")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let tasks = json.as_array().expect("task list should be an array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["filename"], "first.pdf");
}

#[test]
fn test_sanitize_filename() {
    assert_eq!(sanitize_filename("document.pdf"), "document.pdf");
    assert_eq!(sanitize_filename("/path/to/document.pdf"), "document.pdf");
    assert_eq!(sanitize_filename("C:\\Users\\doc.pdf"), "doc.pdf");
    assert_eq!(sanitize_filename("doc.pdf?token=123"), "doc.pdf");
    assert_eq!(sanitize_filename("doc.pdf#page=1"), "doc.pdf");
    assert_eq!(
        sanitize_filename("doc< > : \" | *.pdf"),
        "doc_ _ _ _ _ _.pdf"
    );
    assert_eq!(sanitize_filename("..pdf"), "pdf");
    assert_eq!(sanitize_filename(".pdf"), "pdf");
    assert_eq!(sanitize_filename("test."), "test");
    assert_eq!(sanitize_filename(""), "downloaded");
    assert_eq!(sanitize_filename("   "), "downloaded");
}

#[tokio::test]
async fn test_submit_url_empty() {
    let state = create_test_state();
    let app: Router = create_router(state);

    let request = Request::builder()
        .uri("/api/convert/url")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"url": ""}"#))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_submit_url_invalid_format() {
    let state = create_test_state();
    let app: Router = create_router(state);

    let request = Request::builder()
        .uri("/api/convert/url")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"url": "ftp://example.com/file.pdf"}"#))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_submit_url_adds_extension_from_input_format_override() {
    let state = create_test_state();
    let task_state = state.clone();
    let app: Router = create_router(state);

    let request = Request::builder()
        .uri("/api/convert/url")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"url":"https://example.com/download","config":{"input_format":"json_docling"}}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["filename"], "download.json");
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();
    assert_eq!(task.filename, "download.json");
    assert_eq!(task.config.input_format.as_deref(), Some("json_docling"));
}

#[tokio::test]
async fn test_download_file_streams_completed_output() {
    let dir = tempdir().unwrap();
    let output_path = dir.path().join("result.md");
    tokio::fs::write(&output_path, "# converted").await.unwrap();

    let state = create_test_state();
    let task_id = state
        .create_task("result.pdf".to_string(), TaskConfig::default(), 1)
        .await;
    state
        .set_task_output_path(&task_id, output_path.clone())
        .await;
    state
        .set_task_output(&task_id, format!("/api/download/{}", task_id))
        .await;

    let app: Router = create_router(state);
    let request = Request::builder()
        .uri(format!("/api/download/{}", task_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/markdown; charset=utf-8"
    );
    assert_eq!(
        response.headers().get("content-disposition").unwrap(),
        "attachment; filename=\"result.md\""
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"# converted");
}

#[tokio::test]
async fn test_download_file_uses_html_content_type() {
    let dir = tempdir().unwrap();
    let output_path = dir.path().join("result.html");
    tokio::fs::write(&output_path, "<p>converted</p>")
        .await
        .unwrap();

    let state = create_test_state();
    let task_id = state
        .create_task(
            "result.html".to_string(),
            TaskConfig {
                format: "html".to_string(),
                ..TaskConfig::default()
            },
            1,
        )
        .await;
    state.set_task_output_path(&task_id, output_path).await;
    state
        .set_task_output(&task_id, format!("/api/download/{}", task_id))
        .await;

    let app: Router = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/download/{}", task_id))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
}

#[tokio::test]
async fn test_download_file_uses_doctags_content_type() {
    let dir = tempdir().unwrap();
    let output_path = dir.path().join("result.doctags");
    tokio::fs::write(&output_path, "converted").await.unwrap();

    let state = create_test_state();
    let task_id = state
        .create_task(
            "result.doctags".to_string(),
            TaskConfig {
                format: "doctags".to_string(),
                ..TaskConfig::default()
            },
            1,
        )
        .await;
    state.set_task_output_path(&task_id, output_path).await;
    state
        .set_task_output(&task_id, format!("/api/download/{}", task_id))
        .await;

    let app: Router = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/download/{}", task_id))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/plain; charset=utf-8"
    );
}

#[tokio::test]
async fn test_delete_task_cleans_output_and_work_dir() {
    let state = create_test_state();
    let task_id = state
        .create_task("result.txt".to_string(), TaskConfig::default(), 1)
        .await;
    let work_dir = task_work_dir(&task_id);
    tokio::fs::create_dir_all(&work_dir).await.unwrap();
    let output_path = work_dir.join("result.md");
    tokio::fs::write(&output_path, "# converted").await.unwrap();
    assert!(
        state
            .set_task_output_path(&task_id, output_path.clone())
            .await
    );

    assert!(state.delete_task(&task_id).await);
    assert!(tokio::fs::metadata(&output_path).await.is_err());
    assert!(tokio::fs::metadata(&work_dir).await.is_err());
}

#[tokio::test]
async fn test_download_returns_not_found_after_delete() {
    let dir = tempdir().unwrap();
    let output_path = dir.path().join("result.md");
    tokio::fs::write(&output_path, "# converted").await.unwrap();

    let state = create_test_state();
    let task_id = state
        .create_task("result.pdf".to_string(), TaskConfig::default(), 1)
        .await;
    state
        .set_task_output_path(&task_id, output_path.clone())
        .await;
    state
        .set_task_output(&task_id, format!("/api/download/{}", task_id))
        .await;

    let app: Router = create_router(state);
    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/tasks/{}", task_id))
                .method("DELETE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let download_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/download/{}", task_id))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(download_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_process_url_conversion_forwards_source_without_fetching() {
    let docling = MockDoclingSourceServer::start().await;
    let state = AppState::new(
        docling.base_url.clone(),
        "http://127.0.0.1:8080/v1".to_string(),
        "gpt-4o".to_string(),
        "gpt-4o-mini".to_string(),
        "gpt-4o-mini".to_string(),
    );
    let config = TaskConfig {
        format: "text".to_string(),
        ..TaskConfig::default()
    };
    let task_id = state
        .create_task("notes.md".to_string(), config.clone(), 0)
        .await;
    process_url_conversion(
        state.clone(),
        task_id.clone(),
        "http://127.0.0.1:9/notes.md".to_string(),
        "notes.md".to_string(),
        config,
    )
    .await
    .unwrap();

    let task = state.get_task(&task_id).await.unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.filename, "notes.md");
    assert_eq!(task.total_chunks, 1);
    assert_eq!(task.completed_chunks, 1);
    assert!(task.output_url.is_some());

    let output_path = state.get_task_output_path(&task_id).await.unwrap();
    let output = tokio::fs::read_to_string(&output_path).await.unwrap();
    assert_eq!(output, "hello");

    let requests = docling.requests().await;
    assert_eq!(requests.len(), 3);
    let source_request = String::from_utf8_lossy(&requests[0]);
    assert!(source_request.starts_with("POST /v1/convert/source/async HTTP/1.1"));
    let source_body = source_request.split_once("\r\n\r\n").unwrap().1;
    let source_body: Value = serde_json::from_str(source_body).unwrap();
    assert_eq!(
        source_body["sources"][0]["url"],
        "http://127.0.0.1:9/notes.md"
    );
    assert_eq!(source_body["target"]["kind"], "inbody");

    let _ = state.delete_task(&task_id).await;
}

#[tokio::test]
async fn test_process_url_conversion_forwards_picture_description_preset() {
    let docling = MockDoclingSourceServer::start().await;
    let state = AppState::new(
        docling.base_url.clone(),
        "http://127.0.0.1:8080/v1".to_string(),
        "gpt-4o".to_string(),
        "gpt-4o-mini".to_string(),
        "gpt-4o-mini".to_string(),
    );
    let config = TaskConfig {
        format: "text".to_string(),
        picture_description_preset: Some("smolvlm".to_string()),
        ..TaskConfig::default()
    };
    let task_id = state
        .create_task("notes.md".to_string(), config.clone(), 0)
        .await;
    process_url_conversion(
        state.clone(),
        task_id.clone(),
        "http://127.0.0.1:9/notes.md".to_string(),
        "notes.md".to_string(),
        config,
    )
    .await
    .unwrap();

    let requests = docling.requests().await;
    let source_request = String::from_utf8_lossy(&requests[0]);
    let source_body = source_request.split_once("\r\n\r\n").unwrap().1;
    let source_body: Value = serde_json::from_str(source_body).unwrap();
    assert_eq!(
        source_body["options"]["picture_description_preset"],
        "smolvlm"
    );

    let _ = state.delete_task(&task_id).await;
}

#[tokio::test]
async fn test_process_url_conversion_omits_picture_description_preset_when_unset() {
    let docling = MockDoclingSourceServer::start().await;
    let state = AppState::new(
        docling.base_url.clone(),
        "http://127.0.0.1:8080/v1".to_string(),
        "gpt-4o".to_string(),
        "gpt-4o-mini".to_string(),
        "gpt-4o-mini".to_string(),
    );
    let config = TaskConfig {
        format: "text".to_string(),
        ..TaskConfig::default()
    };
    let task_id = state
        .create_task("notes.md".to_string(), config.clone(), 0)
        .await;
    process_url_conversion(
        state.clone(),
        task_id.clone(),
        "http://127.0.0.1:9/notes.md".to_string(),
        "notes.md".to_string(),
        config,
    )
    .await
    .unwrap();

    let requests = docling.requests().await;
    let source_request = String::from_utf8_lossy(&requests[0]);
    let source_body = source_request.split_once("\r\n\r\n").unwrap().1;
    let source_body: Value = serde_json::from_str(source_body).unwrap();
    assert!(
        source_body["options"]
            .get("picture_description_preset")
            .is_none()
    );

    let _ = state.delete_task(&task_id).await;
}
