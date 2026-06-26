use axum::extract::DefaultBodyLimit;
use axum::serve;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use super::handlers::create_router;
use super::state::AppState;

#[derive(Debug, Clone)]
pub struct WebServerConfig {
    pub docling_base_url: String,
    pub openai_base_url: String,
    pub vlm_pipeline_model: String,
    pub picture_description_model: String,
    pub code_formula_model: String,
    pub host: String,
    pub port: u16,
}

pub async fn run_web_server(config: WebServerConfig) -> std::io::Result<()> {
    let state = AppState::new(
        config.docling_base_url.clone(),
        config.openai_base_url.clone(),
        config.vlm_pipeline_model.clone(),
        config.picture_description_model.clone(),
        config.code_formula_model.clone(),
    );

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = create_router(state.clone())
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr = SocketAddr::new(
        config
            .host
            .parse()
            .unwrap_or_else(|_| "127.0.0.1".parse().unwrap()),
        config.port,
    );

    println!("🌐 Starting web server at http://{}", addr);
    println!("📄 Open http://{} in your browser", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    serve(listener, app)
        .with_graceful_shutdown(async {
            if tokio::signal::ctrl_c().await.is_ok() {
                println!("\n🛑 Shutting down web server...");
            }
        })
        .await?;

    Ok(())
}
