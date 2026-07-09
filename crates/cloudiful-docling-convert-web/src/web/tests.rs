use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    http::{Request, StatusCode},
    response::Response,
    routing::get,
};
use futures::stream;
use serde_json::Value;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::oneshot;
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
        "http://localhost:5001/v1".to_string(),
        "http://localhost:8080/v1".to_string(),
        "gpt-4o".to_string(),
        "gpt-4o-mini".to_string(),
        "gpt-4o-mini".to_string(),
    )
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

async fn spawn_download_server(
    path: &'static str,
    status: StatusCode,
    content_type: &'static str,
    content_length: Option<usize>,
    body: Vec<u8>,
) -> (String, oneshot::Sender<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let body = Arc::new(body);

    let app = Router::new().route(
        path,
        get({
            let body = Arc::clone(&body);
            move || {
                let body = Arc::clone(&body);
                async move {
                    let mut response = Response::builder().status(status);
                    response = response.header("Content-Type", content_type);
                    if let Some(content_length) = content_length {
                        response = response.header("Content-Length", content_length.to_string());
                    }
                    let stream_body = stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::from(
                        body.as_ref().clone(),
                    ))]);
                    response.body(Body::from_stream(stream_body)).unwrap()
                }
            }
        }),
    );

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    (format!("http://{}{}", addr, path), shutdown_tx)
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
    assert_eq!(task.total_chunks, 1);
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
    assert_eq!(task.total_chunks, 1);
}

#[tokio::test]
async fn test_upload_real_pdf_sets_chunk_total() {
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
    assert!(task.total_chunks > 0);
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
            ("pages_per_file", "9"),
            ("split_input", "false"),
            ("split_by_bookmark", "true"),
            ("chunking", "true"),
            ("batch_size", "4"),
        ],
    );
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let task_id = json["task_id"].as_str().unwrap();
    let task = task_state.get_task(task_id).await.unwrap();

    assert_eq!(task.config.format, "json");
    assert_eq!(task.config.pages_per_file, 9);
    assert!(!task.config.split_input);
    assert!(task.config.split_by_bookmark);
    assert!(task.config.chunking);
    assert_eq!(task.config.batch_size, 4);
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
async fn test_process_url_conversion_rejects_oversized_download() {
    let oversized_body = vec![b'a'; 100 * 1024 * 1024 + 1];
    let (url, shutdown) = spawn_download_server(
        "/large.txt",
        StatusCode::OK,
        "text/plain",
        Some(oversized_body.len()),
        oversized_body,
    )
    .await;

    let state = create_test_state();
    let task_id = state
        .create_task("large.txt".to_string(), TaskConfig::default(), 0)
        .await;
    let error = process_url_conversion(
        state,
        task_id,
        url,
        "large.txt".to_string(),
        TaskConfig::default(),
    )
    .await
    .unwrap_err();

    let _ = shutdown.send(());
    assert!(error.to_string().contains("100 MB limit"));
}

#[tokio::test]
async fn test_process_url_conversion_updates_non_pdf_chunks() {
    let (url, shutdown) = spawn_download_server(
        "/notes.txt",
        StatusCode::OK,
        "text/plain",
        Some(5),
        b"hello".to_vec(),
    )
    .await;

    let state = create_test_state();
    let config = TaskConfig {
        format: "text".to_string(),
        ..TaskConfig::default()
    };
    let task_id = state
        .create_task("notes.txt".to_string(), config.clone(), 0)
        .await;
    process_url_conversion(
        state.clone(),
        task_id.clone(),
        url,
        "notes".to_string(),
        config,
    )
    .await
    .unwrap();

    let task = state.get_task(&task_id).await.unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.filename, "notes.txt");
    assert_eq!(task.total_chunks, 1);
    assert_eq!(task.completed_chunks, 1);
    assert!(task.output_url.is_some());

    let output_path = state.get_task_output_path(&task_id).await.unwrap();
    let output = tokio::fs::read_to_string(&output_path).await.unwrap();
    assert_eq!(output, "hello");

    let _ = state.delete_task(&task_id).await;
    let _ = shutdown.send(());
}
