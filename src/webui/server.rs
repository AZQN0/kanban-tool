use anyhow::{Context, Result};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Router};
use std::path::{Component, Path, PathBuf};
use tower_http::trace::TraceLayer;

use super::api::AppState;
use super::sse;

/// Start the WebUI HTTP server.
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `bind` - Bind address (e.g. "127.0.0.1" or "0.0.0.0")
/// * `port` - Port to listen on
pub fn run(project_path: PathBuf, bind: &str, port: u16) -> Result<()> {
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

        let bind_addr = format!("{}:{}", bind, port);
        let listener = tokio::net::TcpListener::bind(&bind_addr)
            .await
            .context(format!("Failed to bind to {}", bind_addr))?;

        let display_url = format!("http://{}:{}", bind, port);
        println!("WebUI running at {}", display_url);

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
        .route(
            "/api/cards",
            get(super::api::list_cards).post(super::api::create_card),
        )
        .route("/api/cards/search", get(super::api::search_cards))
        .route(
            "/api/cards/:id",
            get(super::api::get_card)
                .patch(super::api::update_card)
                .delete(super::api::delete_card),
        )
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
async fn static_file(axum::extract::Path(file): axum::extract::Path<String>) -> Response {
    use axum::http::header;
    let path = match resolve_static_path(&static_dir(), &file) {
        Ok(path) => path,
        Err(err) => return err.into_response(),
    };

    let content_type = mime_guess::from_path(&path).first_or_octet_stream();
    let ct = content_type.as_ref().to_string();

    let body = tokio::fs::read(&path).await.unwrap_or_default();

    let mut response = Response::new(axum::body::Body::from(body));
    if let Ok(hdr) = ct.parse() {
        response.headers_mut().insert(header::CONTENT_TYPE, hdr);
    }
    response
}

#[derive(Debug)]
pub enum StaticFileError {
    Forbidden,
    NotFound,
}

impl StaticFileError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
}

impl IntoResponse for StaticFileError {
    fn into_response(self) -> Response {
        let body = match self {
            Self::Forbidden => "Forbidden",
            Self::NotFound => "File not found",
        };

        (self.status_code(), body.to_string()).into_response()
    }
}

pub fn resolve_static_path(
    static_root: &Path,
    requested: &str,
) -> Result<PathBuf, StaticFileError> {
    let decoded = percent_decode_path(requested).ok_or(StaticFileError::Forbidden)?;
    let requested_path = Path::new(&decoded);

    if requested_path.is_absolute() {
        return Err(StaticFileError::Forbidden);
    }

    for component in requested_path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(StaticFileError::Forbidden);
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    let root = static_root
        .canonicalize()
        .map_err(|_| StaticFileError::NotFound)?;
    let candidate = root.join(requested_path);
    let candidate = candidate
        .canonicalize()
        .map_err(|_| StaticFileError::NotFound)?;

    if !candidate.starts_with(&root) {
        return Err(StaticFileError::Forbidden);
    }

    if !candidate.is_file() {
        return Err(StaticFileError::NotFound);
    }

    Ok(candidate)
}

fn percent_decode_path(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Root handler — serve index.html.
async fn root_handler() -> Response {
    use axum::http::header;
    let path = static_dir().join("index.html");

    if !path.exists() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Frontend files not found.".to_string(),
        )
            .into_response();
    }

    let body = tokio::fs::read(&path).await.unwrap_or_default();

    let mut response = Response::new(axum::body::Body::from(body));
    if let Ok(hdr) = "text/html; charset=utf-8".parse() {
        response.headers_mut().insert(header::CONTENT_TYPE, hdr);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_static_path_rejects_parent_dir_components() {
        let root = static_dir();

        let err = resolve_static_path(&root, "../Cargo.toml").unwrap_err();

        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn resolve_static_path_allows_static_assets() {
        let root = static_dir();

        let path = resolve_static_path(&root, "app.js").unwrap();

        assert!(path.ends_with("app.js"));
    }

    #[test]
    fn resolve_static_path_rejects_directories() {
        let root = static_dir();

        let err = resolve_static_path(&root, ".").unwrap_err();

        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
    }
}
