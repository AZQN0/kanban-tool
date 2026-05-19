#[allow(unused_imports)]
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::broadcast;

use crate::board::card::{Card, Priority};
use crate::board::store::Store;
use crate::board::Board;
use crate::kanban::config;

/// Shared application state for the webui.
#[derive(Clone)]
pub struct AppState {
    pub project_path: PathBuf,
    pub broadcaster: broadcast::Sender<ServerSentEvent>,
}

use super::sse::ServerSentEvent;

/// Error response for API routes.
#[derive(Serialize)]
pub struct ApiError {
    pub error: String,
}

impl ApiError {
    pub fn internal(msg: String) -> Self {
        Self { error: msg }
    }
    pub fn bad_request(msg: String) -> Self {
        Self { error: msg }
    }
    pub fn not_found(msg: String) -> Self {
        Self { error: msg }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = if self.error.starts_with("not_found:") {
            StatusCode::NOT_FOUND
        } else if self.error.starts_with("bad:") {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        (status, Json(self)).into_response()
    }
}

/// Request for creating a card.
#[derive(Deserialize)]
pub struct CreateRequest {
    pub title: String,
    pub description: Option<String>,
    pub column: Option<String>,
    pub priority: Option<String>,
    pub labels: Option<Vec<String>>,
}

/// Request for updating a card.
#[derive(Deserialize)]
pub struct UpdateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub column: Option<String>,
    pub priority: Option<String>,
    pub labels: Option<Vec<String>>,
}

/// Request for moving a card.
#[derive(Deserialize)]
pub struct MoveRequest {
    pub column: String,
}

/// Query parameter for search.
#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

/// Column view with cards, matching the TUI's ColumnView format.
#[derive(Serialize)]
pub struct ColumnView {
    pub name: String,
    pub cards: Vec<Card>,
}

/// Board response grouped by columns.
#[derive(Serialize)]
pub struct BoardResponse {
    pub name: String,
    pub columns: Vec<ColumnView>,
}

/// Helper: open the Store for the project path.
fn open_store(app: &AppState) -> Result<Store, ApiError> {
    let db = config::db_path(&app.project_path);
    Store::open(&db).map_err(|e| ApiError::internal(format!("DB error: {}", e)))
}

/// Helper: get the board for the project path.
fn get_board(app: &AppState) -> Result<Board, ApiError> {
    let store = open_store(app)?;
    store.get_board(&app.project_path.to_string_lossy())
        .map_err(|e| ApiError::not_found(format!("Board not found: {}", e)))
}

// ---------------------------------------------------------------------------
// GET /api/boards — list all boards
// ---------------------------------------------------------------------------

pub async fn list_boards(State(app): State<AppState>) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let db = config::db_path(&app.project_path);
    let store = Store::open(&db)
        .map_err(|e| ApiError::internal(format!("DB error: {}", e)))?;
    
    let boards = store.list_boards()
        .map_err(|e| ApiError::internal(format!("Failed to list boards: {}", e)))?;
    
    let result: Vec<serde_json::Value> = boards.iter().map(|b| {
        serde_json::json!({
            "id": b.id,
            "name": b.name,
            "project_path": b.project_path,
        })
    }).collect();
    
    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// GET /api/cards — get all cards grouped by column
// ---------------------------------------------------------------------------

pub async fn list_cards(State(app): State<AppState>) -> Result<Json<BoardResponse>, ApiError> {
    let board = get_board(&app)?;
    let store = open_store(&app)?;
    
    let columns: Vec<ColumnView> = board.columns.iter().map(|col| {
        let cards = store.list_cards(&board.id, Some(&col.id), None, None, "priority")
            .unwrap_or_default();
        ColumnView {
            name: col.name.clone(),
            cards,
        }
    }).collect();
    
    Ok(Json(BoardResponse {
        name: board.name.clone(),
        columns,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/cards/search?q=term
// ---------------------------------------------------------------------------

pub async fn search_cards(
    State(app): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<Card>>, ApiError> {
    let store = open_store(&app)?;
    let board = get_board(&app)?;
    
    let cards = store.search_cards(&board.id, &query.q)
        .map_err(|e| ApiError::internal(format!("Search failed: {}", e)))?;
    
    Ok(Json(cards))
}

// ---------------------------------------------------------------------------
// GET /api/cards/:id
// ---------------------------------------------------------------------------

pub async fn get_card(
    Path(card_id): Path<String>,
    State(app): State<AppState>,
) -> Result<Json<Card>, ApiError> {
    let store = open_store(&app)?;
    let card = store.get_card(&card_id)
        .map_err(|e| ApiError::not_found(format!("Card not found: {}", e)))?;
    Ok(Json(card))
}

// ---------------------------------------------------------------------------
// POST /api/cards — create a new card
// ---------------------------------------------------------------------------

pub async fn create_card(
    State(app): State<AppState>,
    Json(body): Json<CreateRequest>,
) -> Result<Json<Card>, ApiError> {
    let board = get_board(&app)?;
    let mut store = open_store(&app)?;
    
    // Resolve column
    let col_name = body.column.as_deref().unwrap_or("backlog");
    let target_col = board.columns.iter()
        .find(|c| c.name == col_name)
        .ok_or_else(|| ApiError::bad_request(format!("Column '{}' not found", col_name)))?;
    
    // Resolve priority
    let priority = match body.priority.as_deref() {
        Some(p) => Priority::from_str(p).ok_or_else(|| ApiError::bad_request(format!("Invalid priority: {}", p)))?,
        None => Priority::Backlog,
    };
    
    let labels = body.labels.unwrap_or_default();
    let description = body.description.unwrap_or_default();
    
    // Create card in DB
    let card_file = format!("{}.md", uuid::Uuid::new_v4());
    let card = Card::new(
        &board.id,
        &target_col.id,
        &body.title,
        &description,
        priority,
        labels.clone(),
        PathBuf::from(&card_file),
    );
    
    store.create_card(&card)
        .map_err(|e| ApiError::internal(format!("Failed to create card: {}", e)))?;
    
    // Write markdown file
    crate::markdown::writer::sync_card(&card, &config::cards_dir(&app.project_path))
        .map_err(|e| ApiError::internal(format!("Failed to write markdown: {}", e)))?;
    
    // Broadcast SSE
    app.broadcaster.send(ServerSentEvent::card_created(&card)).ok();
    
    Ok(Json(card))
}

// ---------------------------------------------------------------------------
// PATCH /api/cards/:id — update card fields
// ---------------------------------------------------------------------------

pub async fn update_card(
    Path(card_id): Path<String>,
    State(app): State<AppState>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Card>, ApiError> {
    let mut store = open_store(&app)?;
    let board = get_board(&app)?;
    
    // Verify card exists
    if store.get_card(&card_id).is_err() {
        return Err(ApiError::not_found(format!("Card not found: {card_id}")));
    }
    
    // Resolve optional column
    let new_col_id: Option<&str> = if let Some(ref col_name) = body.column {
        let col = board.columns.iter()
            .find(|c| c.name == *col_name)
            .ok_or_else(|| ApiError::bad_request(format!("Column '{}' not found", col_name)))?;
        Some(col.id.as_str())
    } else {
        None
    };
    
    // Resolve optional priority
    let new_priority_str: Option<String> = if let Some(ref p) = body.priority {
        Some(Priority::from_str(p)
            .ok_or_else(|| ApiError::bad_request(format!("Invalid priority: {}", p)))?
            .to_string())
    } else {
        None
    };
    
    // Update in DB
    store.update_card(
        &card_id,
        body.title.as_deref(),
        body.description.as_deref(),
        new_col_id,
        new_priority_str.as_deref(),
        body.labels.as_ref().map(|l| l.as_slice()),
    ).map_err(|e| ApiError::internal(format!("Failed to update card: {}", e)))?;
    
    // Write updated markdown file
    let updated = store.get_card(&card_id)
        .map_err(|e| ApiError::internal(format!("Failed to read back card: {}", e)))?;
    crate::markdown::writer::sync_card(&updated, &config::cards_dir(&app.project_path))
        .map_err(|e| ApiError::internal(format!("Failed to write markdown: {}", e)))?;
    
    // Broadcast SSE
    app.broadcaster.send(ServerSentEvent::card_updated(&card_id)).ok();
    
    Ok(Json(updated))
}

// ---------------------------------------------------------------------------
// DELETE /api/cards/:id
// ---------------------------------------------------------------------------

pub async fn delete_card(
    Path(card_id): Path<String>,
    State(app): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = open_store(&app)?;
    
    store.delete_card(&card_id)
        .map_err(|e| ApiError::internal(format!("Failed to delete card: {}", e)))?;
    
    // Remove markdown file
    crate::markdown::writer::remove_card_file(&card_id, &config::cards_dir(&app.project_path))
        .ok();
    
    // Broadcast SSE
    app.broadcaster.send(ServerSentEvent::card_deleted(&card_id)).ok();
    
    Ok(Json(serde_json::json!({ "success": true })))
}

// ---------------------------------------------------------------------------
// POST /api/cards/:id/move — move card to column
// ---------------------------------------------------------------------------

pub async fn move_card(
    Path(card_id): Path<String>,
    State(app): State<AppState>,
    Json(body): Json<MoveRequest>,
) -> Result<Json<Card>, ApiError> {
    let mut store = open_store(&app)?;
    let board = get_board(&app)?;
    
    let target_col = board.columns.iter()
        .find(|c| c.name == body.column)
        .ok_or_else(|| ApiError::bad_request(format!("Column '{}' not found", body.column)))?;
    
    // Update DB
    store.transition_card(&card_id, &target_col.id)
        .map_err(|e| ApiError::internal(format!("Failed to move card: {}", e)))?;
    
    // Update markdown file
    let updated = store.get_card(&card_id)
        .map_err(|e| ApiError::internal(format!("Failed to read card after move: {}", e)))?;
    let mut updated_card = updated.clone();
    updated_card.column_id = target_col.id.clone();
    crate::markdown::writer::sync_card(&updated_card, &config::cards_dir(&app.project_path))
        .map_err(|e| ApiError::internal(format!("Failed to update markdown: {}", e)))?;
    
    // Broadcast SSE
    app.broadcaster.send(ServerSentEvent::card_moved(
        &card_id,
        &body.column,
        &target_col.name,
    )).ok();
    
    Ok(Json(updated))
}
