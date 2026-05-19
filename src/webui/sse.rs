use axum::response::sse::Event;
use axum::response::{IntoResponse, Response, Sse};
use axum::Extension;
use serde::Serialize;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;

/// An SSE event sent to connected browsers.
#[derive(Debug, Clone, Serialize)]
pub struct ServerSentEvent {
    pub event: String,
    pub data: serde_json::Value,
}

impl ServerSentEvent {
    pub fn card_created(card: &crate::board::card::Card) -> Self {
        Self {
            event: "card_created".to_string(),
            data: serde_json::to_value(card).unwrap_or_default(),
        }
    }

    pub fn card_moved(card_id: &str, new_column: &str, new_column_name: &str) -> Self {
        Self {
            event: "card_moved".to_string(),
            data: serde_json::json!({
                "card_id": card_id,
                "new_column": new_column,
                "new_column_name": new_column_name
            }),
        }
    }

    pub fn card_updated(card_id: &str) -> Self {
        Self {
            event: "card_updated".to_string(),
            data: serde_json::json!({ "card_id": card_id }),
        }
    }

    pub fn card_deleted(card_id: &str) -> Self {
        Self {
            event: "card_deleted".to_string(),
            data: serde_json::json!({ "card_id": card_id }),
        }
    }
}

/// Shared state for SSE.
pub struct AppState {
    pub broadcaster: broadcast::Sender<ServerSentEvent>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { broadcaster: tx }
    }
}

/// SSE endpoint handler — serves `text/event-stream` to clients.
pub async fn sse_handler(
    Extension(app): Extension<AppState>,
) -> Response {
    let stream = async_stream::stream! {
        let mut rx = app.broadcaster.subscribe();
        loop {
            match rx.recv().await {
                Ok(sse_event) => {
                    let json_data = serde_json::to_string(&sse_event.data)
                        .unwrap_or_default();
                    yield Ok::<_, axum::BoxError>(Event::default()
                        .event(sse_event.event)
                        .data(json_data));
                }
                Err(broadcast::error::RecvError::Lagged(_n)) => {
                    eprintln!("SSE client lagged, skipping");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).into_response()
}
