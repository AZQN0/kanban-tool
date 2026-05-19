use anyhow::{Context, Result};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Router};
use std::path::PathBuf;
use tower_http::trace::TraceLayer;

use super::api::AppState;
use super::sse;

/// Start the WebUI HTTP server on localhost:9876.
pub fn run(project_path: PathBuf) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")?;

    rt.block_on(async {
        let (broadcaster, _) = tokio::sync::broadcast::channel(100);
        
        let app_state = super::api::AppState {
            project_path,
            broadcaster,
        };
        
        let app = build_app(app_state.clone()).await;
        
        let listener = tokio::net::TcpListener::bind("127.0.0.1:9876")
            .await
            .context("Failed to bind to 127.0.0.1:9876")?;
        
        println!("WebUI running at http://localhost:9876");
        
        axum::serve(listener, app)
            .await
            .context("WebUI server failed")?;
        
        Ok::<_, anyhow::Error>(())
    })
}

async fn build_app(app_state: AppState) -> Router {
    Router::new()
        // REST API routes
        .route("/api/boards", get(super::api::list_boards))
        .route("/api/cards", get(super::api::list_cards).post(super::api::create_card))
        .route("/api/cards/search", get(super::api::search_cards))
        .route("/api/cards/:id", get(super::api::get_card)
            .patch(super::api::update_card)
            .delete(super::api::delete_card))
        .route("/api/cards/:id/move", post(super::api::move_card))
        // SSE endpoint
        .route("/api/events", get(sse::sse_handler))
        // Static files
        .route("/static/*file", get(static_file))
        // Root — serve index.html
        .route("/", get(root_handler))
        .with_state(app_state.clone())
        .layer(Extension(app_state))
        .layer(TraceLayer::new_for_http())
}

/// Get the static files directory.
/// Checks: CWD/webui/static/, then binary-dir/../../webui/static/.
pub fn static_dir() -> PathBuf {
    // Check CWD first (when run from project root)
    let cwd_candidate = PathBuf::from("webui/static");
    if cwd_candidate.exists() {
        return cwd_candidate;
    }
    
    // Check relative to binary (when run from a project directory)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("../../webui/static");
            if let Ok(canonical) = candidate.canonicalize() {
                if canonical.exists() {
                    return canonical;
                }
            }
        }
    }
    
    // Fallback
    PathBuf::from("webui/static")
}

/// Serve static files from webui/static/.
async fn static_file(
    axum::extract::Path(file): axum::extract::Path<String>,
) -> Response {
    use axum::http::header;
    let path = static_dir().join(&file);
    
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "File not found".to_string()).into_response();
    }
    
    let content_type = mime_guess::from_path(&path)
        .first_or_octet_stream();
    let ct = content_type.as_ref().to_string();
    
    let body = tokio::fs::read(&path).await.unwrap_or_default();
    
    let mut response = Response::new(axum::body::Body::from(body));
    if let Ok(hdr) = ct.parse() {
        response.headers_mut().insert(header::CONTENT_TYPE, hdr);
    }
    response
}

/// Root handler — serve index.html.
async fn root_handler() -> Response {
    use axum::http::header;
    let path = static_dir().join("index.html");
    
    if !path.exists() {
        return (StatusCode::SERVICE_UNAVAILABLE, "Frontend files not found.".to_string()).into_response();
    }
    
    let body = tokio::fs::read(&path).await.unwrap_or_default();
    
    let mut response = Response::new(axum::body::Body::from(body));
    if let Ok(hdr) = "text/html; charset=utf-8".parse() {
        response.headers_mut().insert(header::CONTENT_TYPE, hdr);
    }
    response
}
