# WebUI Feature Design

> A browser-based kanban board companion to the TUI, with full feature parity. Embedded HTTP server in the same binary, served via `kanban webui`.

**Goal:** Add an optional WebUI interface that mirrors the TUI's functionality — 3-panel board view, card navigation, move, edit, delete, search, project switching, plus drag-and-drop card movement — with live updates via SSE.

**Architecture:** Axum 0.8 HTTP server serves vanilla HTML/CSS/JS frontend from a `webui/static/` directory. REST API endpoints call existing `Store` methods. SSE broadcasts card mutations to connected browsers. Feature-gated behind `--features webui`.

**Dependencies:** `axum` 0.8, `tower-http` (fs, trace), `mime_guess`, `async-stream`, `tokio-stream`. No external frontend dependencies.

---

## Project Structure

```
kanban-tool/
├── src/
│   ├── webui/                    ← New module (feature-gated)
│   │   ├── mod.rs                ← Module declarations
│   │   ├── server.rs             ← Axum server setup, route registration, static file serving
│   │   ├── api.rs                ← REST handler functions (thin wrappers around Store)
│   │   └── sse.rs                ← SSE event broadcaster (tokio::sync::broadcast + async-stream)
│   ├── main.rs                   ← Add WebUI subcommand dispatch
│   └── ... (all existing modules unchanged)
├── webui/                        ← New frontend directory
│   └── static/
│       ├── index.html            ← Main page, 3-panel layout
│       ├── style.css             ← Dark terminal-inspired theme
│       └── app.js                ← Vanilla JS: REST calls, SSE, DOM updates, drag-and-drop
├── Cargo.toml                    ← Add axum + tower-http + mime_guess + async-stream + tokio-stream
└── ... (everything else unchanged)
```

**Feature flag:** `webui` feature gates the `src/webui/` module and its dependencies.
```toml
[features]
default = []
webui = ["dep:axum", "dep:tower-http", "dep:mime_guess", "dep:async-stream", "dep:tokio-stream"]
```

---

## CLI Entry

```bash
kanban webui
```

Runs an HTTP server on `localhost:9876` (kanban on phone keypad mnemonic). Starts from within a project directory (like `kanban board` for TUI). Uses the project's `.kanban/` SQLite database.

**Static file resolution:** The server first checks `webui/static/` relative to the current working directory, then falls back to resolving the path relative to the binary's location (`binary/../webui/static/`).

---

## API Design

### REST Endpoints

| Method | Endpoint | Description | Request Body | Response |
|--------|----------|-------------|--------------|----------|
| `GET` | `/api/boards` | List all boards | — | `[{id, name, project_path}]` |
| `GET` | `/api/cards` | Get all cards grouped by column | — | `{name: string, columns: [{name, cards: []}]}` |
| `GET` | `/api/cards/search?q=term` | Search cards across columns | Query param `q` | `[{card objects}]` |
| `GET` | `/api/cards/:id` | Get single card detail | — | Card object |
| `POST` | `/api/cards` | Create a new card | `{title, description?, column?, priority?, labels?}` | Created card |
| `PATCH` | `/api/cards/:id` | Update card fields | `{title?, description?, priority?, column?, labels?}` | Updated card |
| `DELETE` | `/api/cards/:id` | Delete a card | — | `{success: true}` |
| `POST` | `/api/cards/:id/move` | Move card to column | `{column: "done"}` | Updated card |
| `GET` | `/api/events` | SSE endpoint for live updates | — | `text/event-stream` |

### SSE Events

Server broadcasts on a `tokio::sync::broadcast::Sender<ServerSentEvent>` with channel capacity 100. Each connected browser gets its own receiver via `subscribe()`.

**Event types:**

```
event: card_created
data: {"id": "...", "board_id": "...", "title": "...", "column_id": "...", ...}

event: card_moved
data: {"card_id": "...", "new_column": "done", "new_column_name": "Done"}

event: card_updated
data: {"card_id": "..."}

event: card_deleted
data: {"card_id": "..."}
```

**Error handling:**
- Store errors → 500 JSON `{"error": "message"}`
- Not found → 404 JSON `{"error": "message"}`
- Invalid column/priority → 400 JSON `{"error": "message"}`
- Lagged SSE clients → event skipped (RecvError::Lagged handled)
- Disconnected clients → stream breaks out of loop (RecvError::Closed handled)

---

## Frontend Design

### Layout (mirrors TUI 3-panel)

```
┌─────────────────────────────────────────────────────────┐
│ 📋 Project: my-project          Cards: 12      🔍 Search │  ← Top bar
├──────────┬──────────────────────────┬───────────────────┤
│ Columns  │ Todo (3)                 │ Fix auth token    │
│          │                          │ refresh           │
│ ▶ backlog│ • a1b2c3d4 Fix login     │ ───────────────── │
│   todo   │ • e5f6g7h8 Add settings  │ Priority: high    │
│   done   │ <card-9abc> Search UX   │ Labels: bug, auth │
│          │                          │ ───────────────── │
│          │                          │ Description:      │
│          │                          │ When the access... │
│          │                          │                    │
├──────────┴──────────────────────────┴───────────────────┤
│ Focus: Cards  | ↑↓ Nav │ Enter Focus │ m Move │ e Edit │ D Delete │ P Proj │ / Search │ q Quit│  ← Bottom bar
└─────────────────────────────────────────────────────────┘
```

### Interaction Model

| Action | Keyboard | Mouse |
|--------|----------|-------|
| Navigate cards | ↑↓ / jk | Click on a card |
| Switch columns | ←→ / hl | Click column header |
| Focus detail | Enter | Click a card (same as navigate) |
| Move card | m → select column | **Drag card → drop on target column** |
| Edit card | e | Not available via mouse (edit is keyboard-only) |
| Delete card | D | Not available via mouse (delete is keyboard-only) |
| Search | / | Click search icon in top bar |
| Switch project | P | Click project dropdown |
| Close modal | Esc | Click outside modal |

### Drag-and-Drop

- HTML5 native drag-and-drop API (no external library)
- Each card `<div>` has `draggable=true`
- `dragstart` sets `dataTransfer` with card ID
- Cards panel is a drop target — determines target column from `state.currentColumnIdx`
- On drop: JS sends `POST /api/cards/:id/move`, server updates DB and markdown file, then broadcasts SSE
- Visual feedback: cards panel gets `outline: 2px solid var(--border-active)` on dragover

### Edit Flow

Pressing `e` replaces the detail panel with inline editing:
- Title input field (text)
- Priority selector dropdown (backlog/low/medium/high/urgent)
- Labels input (comma-separated)
- Description `<textarea>` (full markdown)
- Save / Cancel buttons

Save sends `PATCH /api/cards/:id`. Cancel reverts to detail view. No modal — edit is inline in the detail panel.

### Styling

Dark terminal-inspired theme (Tokyo Night palette):
- Background: `#1a1b26`
- Borders: `#3b4261` (subtle), `#7aa2f7` (active)
- Text: `#c0caf5`
- Highlight: `#e0af68` (yellow)
- Priority colors: Urgent = `#f7768e` (red), High = `#bb9af7` (magenta), Medium = `#e0af68` (yellow), Low = `#9ece6a` (green), Backlog = `#565f89` (gray)
- Font: Monospace stack — "JetBrains Mono", "Fira Code", "Cascadia Code", "SF Mono", monospace
- No external CSS frameworks — hand-crafted CSS variables and flexbox

---

## Rust Backend Implementation

### `src/webui/server.rs`

```rust
use axum::{Extension, Router, http::StatusCode, response::{IntoResponse, Response}};
use axum::routing::{get};
use tower_http::trace::TraceLayer;
use std::path::PathBuf;

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
        
        axum::serve(listener, app).await.context("WebUI server failed")?;
        
        Ok::<_, anyhow::Error>(())
    })
}

async fn build_app(app_state: AppState) -> Router {
    Router::new()
        .route("/api/boards", get(super::api::list_boards))
        .route("/api/cards", get(super::api::list_cards).post(super::api::create_card))
        .route("/api/cards/search", get(super::api::search_cards))
        .route("/api/cards/:id", get(super::api::get_card)
            .patch(super::api::update_card)
            .delete(super::api::delete_card))
        .route("/api/cards/:id/move", post(super::api::move_card))
        .route("/api/events", get(sse::sse_handler))
        .route("/static/*file", get(static_file))
        .route("/", get(root_handler))
        .with_state(app_state.clone())
        .layer(Extension(app_state))
        .layer(TraceLayer::new_for_http())
}

/// Get static files directory — checks CWD then binary-relative path.
pub fn static_dir() -> PathBuf {
    // Check CWD first (when run from project root)
    let cwd_candidate = PathBuf::from("webui/static");
    if cwd_candidate.exists() { return cwd_candidate; }
    
    // Check relative to binary (when run from a project directory)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("../../webui/static");
            if let Ok(canonical) = candidate.canonicalize() {
                if canonical.exists() { return canonical; }
            }
        }
    }
    PathBuf::from("webui/static")
}
```

### `src/webui/api.rs`

Thin standalone functions using `State<AppState>`. Shared state:

```rust
#[derive(Clone)]
pub struct AppState {
    pub project_path: PathBuf,
    pub broadcaster: broadcast::Sender<ServerSentEvent>,
}
```

**Request types:**
```rust
pub struct CreateRequest {
    pub title: String,
    pub description: Option<String>,
    pub column: Option<String>,
    pub priority: Option<String>,
    pub labels: Option<Vec<String>>,
}

pub struct UpdateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub column: Option<String>,
    pub priority: Option<String>,
    pub labels: Option<Vec<String>>,
}

pub struct MoveRequest {
    pub column: String,
}

pub struct SearchQuery {
    pub q: String,
}
```

**Response types:**
```rust
pub struct ColumnView {
    pub name: String,
    pub cards: Vec<Card>,
}

pub struct BoardResponse {
    pub name: String,
    pub columns: Vec<ColumnView>,
}
```

**Error type:**
```rust
pub struct ApiError { pub error: String }
impl ApiError {
    pub fn internal(msg: String) -> Self { Self { error: msg } }
    pub fn bad_request(msg: String) -> Self { Self { error: msg } }
    pub fn not_found(msg: String) -> Self { Self { error: msg } }
}
// Status mapping: "not_found:" → 404, "bad:" → 400, else → 500
```

**Handler signatures:**
```rust
pub async fn list_boards(State(app): State<AppState>) -> Result<Json<Vec<Value>>, ApiError>
pub async fn list_cards(State(app): State<AppState>) -> Result<Json<BoardResponse>, ApiError>
pub async fn search_cards(State(app): State<AppState>, Query(query): Query<SearchQuery>) -> Result<Json<Vec<Card>>, ApiError>
pub async fn get_card(Path(card_id): Path<String>, State(app): State<AppState>) -> Result<Json<Card>, ApiError>
pub async fn create_card(State(app): State<AppState>, Json(body): Json<CreateRequest>) -> Result<Json<Card>, ApiError>
pub async fn update_card(Path(card_id): Path<String>, State(app): State<AppState>, Json(body): Json<UpdateRequest>) -> Result<Json<Card>, ApiError>
pub async fn delete_card(Path(card_id): Path<String>, State(app): State<AppState>) -> Result<Json<Value>, ApiError>
pub async fn move_card(Path(card_id): Path<String>, State(app): State<AppState>, Json(body): Json<MoveRequest>) -> Result<Json<Card>, ApiError>
```

Each mutation handler follows the same pattern:
1. `open_store(&app)` — opens `Store` from `project_path`
2. Call the appropriate `store.*()` method
3. Sync markdown file via `markdown::writer::sync_card()` or `remove_card_file()`
4. `app.broadcaster.send(ServerSentEvent::...)` — broadcast SSE event
5. Return JSON response

**Static file serving:**
```rust
async fn static_file(Path(file): Path<String>) -> Response {
    let path = static_dir().join(&file);
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "File not found".to_string()).into_response();
    }
    let content_type = mime_guess::from_path(&path).first_or_octet_stream();
    let body = tokio::fs::read(&path).await.unwrap_or_default();
    let mut response = Response::new(axum::body::Body::from(body));
    response.headers_mut().insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    response
}

async fn root_handler() -> Response {
    // Serves webui/static/index.html with text/html content type
    // Returns 503 if file not found
}
```

### `src/webui/sse.rs`

```rust
/// An SSE event sent to connected browsers.
#[derive(Debug, Clone, Serialize)]
pub struct ServerSentEvent {
    pub event: String,   // "card_created", "card_moved", "card_updated", "card_deleted"
    pub data: serde_json::Value,
}
```

**Constructor methods:**
```rust
impl ServerSentEvent {
    pub fn card_created(card: &Card) -> Self;
    pub fn card_moved(card_id: &str, new_column: &str, new_column_name: &str) -> Self;
    pub fn card_updated(card_id: &str) -> Self;
    pub fn card_deleted(card_id: &str) -> Self;
}
```

**Handler — SSE endpoint:**
```rust
pub async fn sse_handler(
    Extension(state): Extension<AppState>,
) -> Response {
    let stream = async_stream::stream! {
        let mut rx = state.broadcaster.subscribe();
        loop {
            match rx.recv().await {
                Ok(sse_event) => {
                    let json_data = serde_json::to_string(&sse_event.data).unwrap_or_default();
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
```

---

## Dependencies

**New dependencies in `Cargo.toml` (feature-gated):**
```toml
[dependencies]
axum = { version = "0.8", optional = true }
tower-http = { version = "0.5", features = ["fs", "trace"], optional = true }
mime_guess = { version = "2", optional = true }
async-stream = { version = "0.3", optional = true }
tokio-stream = { version = "0.1", optional = true }

[features]
default = []
webui = ["dep:axum", "dep:tower-http", "dep:mime_guess", "dep:async-stream", "dep:tokio-stream"]
```

**No new frontend dependencies** — vanilla HTML/CSS/JS only.

---

## Data Flow

```
Browser                    Axum Server                    Store (SQLite)
   │                           │                               │
   │  GET /api/cards           │                               │
   ├──────────────────────────►│                               │
   │                           ├─ store.list_cards() ─────────►│
   │                           │                               │
   │  Cards JSON + columns     │                               │
   │◄──────────────────────────┤                               │
   │                           │                               │
   │  GET /api/events          │                               │
   ├──────────────────────────►│                               │
   │                           │  broadcast::Receiver created  │
   │                           │  └─→ per-client subscription  │
   │                           │                               │
   │  (user drags card)        │                               │
   │  POST /api/cards/:id/move │                               │
   ├──────────────────────────►│                               │
   │                           ├─ store.transition_card() ────►│
   │                           │  markdown::writer sync        │
   │                           │  broadcast("card_moved")      │
   │                           │  └─→ all SSE receivers        │
   │  Event: {"type":"..."}    │                               │
   │◄──────────────────────────┤                               │
   │                           │                               │
   │  (JS updates DOM)         │                               │
   └──────────────────────────►│                               │
```

## Live Update Strategy

The frontend uses **dual update strategy**:
1. **SSE** — primary real-time channel for card mutations. Each event triggers a targeted DOM refresh of the affected card/column.
2. **Polling fallback** — every 3 seconds, the frontend compares the current column/card ID structure against the API response. If they differ, it does a full `loadBoard()` refresh. This handles cases where SSE is unavailable (browser limitations, network issues).

---

## File Served at Runtime

Frontend files are read from `webui/static/` at runtime (no embedding). The server searches in this order:
1. `webui/static/` relative to current working directory (when run from project root)
2. `binary/../webui/static/` — resolves `../../webui/static` from binary's parent directory (when run from a project directory while binary is in `target/debug/`)
3. Fallback to `webui/static/` relative to CWD

During development, edit files and refresh the browser. During `cargo install`, the `webui/static/` directory must be available relative to the binary or CWD.

---

## Self-Review Checklist

1. **Placeholder scan:** No TBD, TODO, or vague requirements. All endpoints specified with methods, paths, request/response shapes.
2. **Internal consistency:** API handlers call existing `Store` methods — no duplicate data logic. SSE broadcasts after every mutation. Frontend state mirrors TUI state (columns, cards, detail_card, focus).
3. **Scope check:** Focused single feature — WebUI companion to TUI. Not a full web app with auth, collaboration, or notifications. All TUI features included (full parity).
4. **Ambiguity check:** 
   - Port is explicit: 9876
   - SSE event types are explicit: `card_created`, `card_moved`, `card_updated`, `card_deleted`
   - Drag-and-drop uses HTML5 native API, no external library
   - Edit is inline (textarea + inputs in detail panel), not modal
   - Project switching fetches from existing `store.list_boards()` — reuses existing data
   - Static file path resolution handles both CWD and binary-relative locations
5. **Error handling:** All handlers return typed `ApiError` (internal/404/400). SSE lagged clients are skipped gracefully. Disconnected clients close their streams.
