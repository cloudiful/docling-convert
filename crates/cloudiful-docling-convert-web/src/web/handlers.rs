use axum::{
    Json, Router,
    body::Bytes,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use tokio_util::io::ReaderStream;
use tower_http::services::ServeDir;

use super::conversion::{
    estimated_total_chunks_for_input, process_file_conversion, process_url_conversion,
    spawn_conversion_task,
};
use super::state::{AppState, ConversionTask, TaskConfig, TaskStatus};
use super::support::{
    output_content_type, parse_input_format, resolve_input_kind, sanitize_filename,
};
use super::task_config::TaskConfigInput;
use cloudiful_docling_convert::InputDocument;

pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn list_tasks(State(state): State<AppState>) -> Json<Vec<ConversionTask>> {
    Json(state.list_tasks().await)
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> std::result::Result<Json<ConversionTask>, StatusCode> {
    state
        .get_task(&task_id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn delete_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> std::result::Result<StatusCode, StatusCode> {
    if state.delete_task(&task_id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(serde::Deserialize)]
pub struct UrlRequest {
    pub url: String,
    pub config: Option<TaskConfigInput>,
}

pub async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let upload = parse_upload_request(&mut multipart).await?;
    let file_name = upload.file_name;
    let media_type = upload.media_type;
    let data = upload.data;
    let config = upload.config;
    let input_kind = resolve_input_kind(&file_name, &media_type, config.input_format.as_deref())?;
    let input = InputDocument::new(
        file_name.clone(),
        input_kind.canonical_media_type(&file_name, Some(&media_type)),
        data,
    )
    .with_input_kind(input_kind);
    let total_chunks = estimated_total_chunks_for_input(&input, &config);

    let task_id = state
        .create_task(file_name.clone(), config.clone(), total_chunks)
        .await;

    spawn_file_task(state.clone(), task_id.clone(), input, config);

    Ok(Json(serde_json::json!({
        "task_id": task_id,
        "filename": file_name,
        "message": "File uploaded successfully"
    })))
}

pub async fn submit_url(
    State(state): State<AppState>,
    Json(payload): Json<UrlRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    if payload.url.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let url = payload.url.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let config = resolve_task_config(payload.config)?;
    let file_name = derive_url_filename(&url, config.input_format.as_deref())?;
    let task_id = state
        .create_task(file_name.clone(), config.clone(), 0)
        .await;

    spawn_url_task(
        state.clone(),
        task_id.clone(),
        url,
        file_name.clone(),
        config,
    );

    Ok(Json(serde_json::json!({
        "task_id": task_id,
        "filename": file_name,
        "message": "URL submitted successfully"
    })))
}

pub async fn download_file(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> std::result::Result<Response, StatusCode> {
    let task = state
        .get_task(&task_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    if task.status != TaskStatus::Completed || task.output_url.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let output_path = state
        .get_task_output_path(&task_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let file = tokio::fs::File::open(&output_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let file_len = file
        .metadata()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let filename = output_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let stream = ReaderStream::new(file);

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", output_content_type(&task.config.format))
        .header("Content-Length", file_len.len())
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(axum::body::Body::from_stream(stream))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn create_router(state: AppState) -> Router {
    let api_routes = Router::new()
        .route("/tasks", get(list_tasks))
        .route("/tasks/{id}", get(get_task))
        .route("/tasks/{id}", delete(delete_task))
        .route("/upload", post(upload_file))
        .route("/convert/url", post(submit_url))
        .route("/download/{id}", get(download_file))
        .with_state(state.clone());

    Router::new()
        .route("/health", get(health_check))
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new("web/dist"))
        .with_state(state)
}

struct UploadRequest {
    file_name: String,
    media_type: String,
    data: Bytes,
    config: TaskConfig,
}

async fn parse_upload_request(multipart: &mut Multipart) -> Result<UploadRequest, StatusCode> {
    let mut file_name = None;
    let mut media_type = None;
    let mut data = None;
    let mut config_fields = std::collections::HashMap::new();

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => return Err(StatusCode::BAD_REQUEST),
        };

        let Some(field_name) = field.name().map(str::to_owned) else {
            continue;
        };

        if field_name == "file" {
            file_name = Some(sanitize_filename(field.file_name().unwrap_or("upload")));
            media_type = Some(
                field
                    .content_type()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
            );
            data = Some(field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?);
            continue;
        }

        let value = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
        config_fields.insert(field_name, value);
    }

    let config = resolve_task_config(Some(TaskConfigInput::from_multipart_fields(
        &config_fields,
    )?))?;

    Ok(UploadRequest {
        file_name: file_name.ok_or(StatusCode::BAD_REQUEST)?,
        media_type: media_type.ok_or(StatusCode::BAD_REQUEST)?,
        data: data.ok_or(StatusCode::BAD_REQUEST)?,
        config,
    })
}

fn resolve_task_config(config: Option<TaskConfigInput>) -> Result<TaskConfig, StatusCode> {
    config.unwrap_or_default().resolve()
}

fn derive_url_filename(url: &str, input_format: Option<&str>) -> Result<String, StatusCode> {
    let file_name = url.split('/').last().unwrap_or("downloaded");
    let file_name = file_name
        .split('?')
        .next()
        .unwrap_or("downloaded")
        .split('#')
        .next()
        .unwrap_or("downloaded");

    let mut file_name = if file_name.is_empty() {
        "downloaded".to_string()
    } else {
        sanitize_filename(file_name)
    };

    if std::path::Path::new(&file_name).extension().is_none() {
        if let Some(input_format) = input_format {
            let input_kind =
                parse_input_format(input_format).map_err(|_| StatusCode::BAD_REQUEST)?;
            file_name = format!("{}.{}", file_name, input_kind.default_extension());
        }
    }

    Ok(file_name)
}

fn spawn_file_task(state: AppState, task_id: String, input: InputDocument, config: TaskConfig) {
    spawn_conversion_task(
        state.clone(),
        task_id.clone(),
        process_file_conversion(state, task_id, input, config),
    );
}

fn spawn_url_task(
    state: AppState,
    task_id: String,
    url: String,
    file_name: String,
    config: TaskConfig,
) {
    spawn_conversion_task(
        state.clone(),
        task_id.clone(),
        process_url_conversion(state, task_id, url, file_name, config),
    );
}
