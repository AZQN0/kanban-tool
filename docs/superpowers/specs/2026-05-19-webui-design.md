# WebUI Feature Design

> A browser-based kanban board companion to the TUI, with full feature parity. Embedded HTTP server in the same binary, served via `kanban webui`.

**Goal:** Add an optional WebUI interface that mirrors the TUI's functionality — 3-panel board view, card navigation, move, edit, delete, search, project switching, plus drag-and-drop card movement — with live updates via SSE.

**Architecture:** Axum HTTP server serves vanilla HTML/CSS/JS frontend from a `webui/static/` directory. REST API endpoints call existing `Store` methods. SSE broadcasts card mutations to connected browsers. Feature-gated behind `--features webui`.

---

## Project Structure

```
kanban-tool/
├── src/
│   ├── webui/                    ← New module (feature-gated)
│   │   ├── mod.rs                ← Module declaration
│   │   ├── server.rs             ← Axum server setup, route registration, SSE broadcast
│   │   ├── api.rs                ← REST handler functions (thin wrappers around Store)
│   │   └── sse.rs                ← SSE event broadcaster (tokio::sync::broadcast)
│   ├── main.rs                   ← Add WebUI subcommand dispatch
│   └── ... (all existing modules unchanged)
├── webui/                        ← New frontend directory
│   └── static/
│       ├── index.html            ← Main page, 3-panel layout
│       ├── style.css             ← Dark terminal-inspired theme
│       └── app.js                ← Vanilla JS: REST calls, SSE, DOM updates
├── Cargo.toml                    ← Add axum + tower-http + mime_guess deps, feature flag
└── ... (everything else unchanged)
```

**Feature flag:** `webui` feature gates the `src/webui/` module and its dependencies.
```toml
[features]
default = []
webui = ["dep:axum", "dep:tower-http", "dep:mime_guess"]
```

---

## CLI Entry

```bash
kanban webui
```

Runs an HTTP server on `localhost:9876` (kanban on phone keypad mnemonic). Opens from within a project directory (like `kanban board` for TUI). Uses the project's `.kanban/` database.

---

## API Design

### REST Endpoints

| Method | Endpoint | Description | Request Body | Response |
|--------|----------|-------------|--------------|----------|
| `GET` | `/api/boards` | List all boards | — | `[{id, name, project_path}]` |
| `GET` | `/api/cards` | Get all cards grouped by column | — | `{columns: [{name, cards: []}]}` |
| `GET` | `/api/cards/search?q=term` | Search cards | Query param `q` | `{cards: []}` |
| `GET` | `/api/cards/:id` | Get single card detail | — | Card object |
| `POST` | `/api/cards` | Create a new card | `{title, description?, priority?, column?, labels?}` | Created card |
| `PATCH` | `/api/cards/:id` | Update card fields | `{title?, description?, priority?, column?, labels?}` | Updated card |
| `DELETE` | `/api/cards/:id` | Delete a card | — | `{success: true}` |
| `POST` | `/api/cards/:id/move` | Move card to column | `{column: "done"}` | Updated card |
| `GET` | `/api/events` | SSE endpoint for live updates | — | `text/event-stream` |

### SSE Events

Server broadcasts on a `tokio::sync::broadcast` channel. Each connected browser gets its own receiver.

Event format:
```
event: card_created
data: {"card": {...}}

event: card_moved
data: {"card_id": "...", "new_column": "done", "new_column_name": "Done"}

event: card_updated
data: {"card_id": "..."}

event: card_deleted
data: {"card_id": "..."}
```

**Error handling:**
- Store errors → 500 JSON `{"error": "message"}`
- Not found → 404
- Invalid column → 400 `{"error": "Column X not found"}`
- SSE disconnection → client auto-reconnects with exponential backoff (3s → 6s → 12s, max 30s)

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
│ ↑↓ Nav │ Enter Focus │ m Move │ e Edit │ D Delete │ P Proj │ q Quit│  ← Bottom bar
└─────────────────────────────────────────────────────────┘
```

### Interaction Model

| Action | Keyboard | Mouse |
|--------|----------|-------|
| Navigate cards | ↑↓ / jk | Click on a card |
| Switch columns | ←→ / hl | Click column header |
| Focus detail | Enter | Click a card |
| Move card | m → select column | **Drag card → drop on target column** |
| Edit card | e | Double-click card / click edit icon |
| Delete card | D | Click delete icon (with confirmation) |
| Search | / | Click search icon in top bar |
| Switch project | P | Click project dropdown |
| Close modal | Esc | Click outside modal / X button |

### Drag-and-Drop

- HTML5 native drag-and-drop API (no external library)
- Cards are `draggable`, column headers are drop targets
- Visual feedback: card lifts on drag, target column highlights on hover
- On drop: JS sends `POST /api/cards/:id/move`, server updates and broadcasts SSE

### Edit Flow

Pressing `e` or double-clicking a card replaces the detail panel with:
- Title input field
- Description `<textarea>` (full markdown)
- Priority selector dropdown
- Labels input (comma-separated)
- Save / Cancel buttons

Save sends `PATCH /api/cards/:id`. Cancel reverts to detail view.

### Styling

Dark terminal-inspired theme:
- Background: `#1a1b26` (tokyo-night dark)
- Borders: `#7aa2f7` (cyan)
- Highlights: `#e0af68` (yellow/orange)
- Priority colors: Urgent = red, High = magenta, Medium = yellow, Low = green, Backlog = gray
- Monospace font (matching TUI feel)
- No external CSS frameworks — hand-crafted

---

## Rust Backend Implementation

### `src/webui/server.rs`

```rust
pub async fn run(project_path: PathBuf) -> Result<()> {
    let broadcaster = broadcast::channel(100).0;
    let api = Arc::new(Api::new(project_path.clone()));
    
    let app = Router::new()
        .route("/static/:file*", get(static_files))
        .route("/api/boards", get(api.list_boards))
        .route("/api/cards", get(api.list_cards).post(api.create_card))
        .route("/api/cards/search", get(api.search_cards))
        .route("/api/cards/:id", get(api.get_card).patch(api.update_card).delete(api.delete_card))
        .route("/api/cards/:id/move", post(api.move_card))
        .route("/api/events", get(sse_handler))
        .with_state(api)
        .with_state(broadcaster.clone());
    
    let listener = TcpListener::bind("127.0.0.1:9876").await?;
    axum::serve(listener, app).await?;
}
```

### `src/webui/api.rs`

Thin wrappers around existing `Store` methods:

```rust
pub struct Api {
    project_path: PathBuf,
    // Shared state if needed
}

impl Api {
    pub fn new(project_path: PathBuf) -> Self;
    pub async fn list_cards(&self) -> Json<Value>;
    pub async fn get_card(&self, Path(id): Path<String>) -> Result<Json<Value>, AppError>;
    pub async fn create_card(&self, Json(body): Json<Value>) -> Result<Json<Value>, AppError>;
    pub async fn update_card(&self, Path(id): Path<String>, Json(body): Json<Value>) -> Result<Json<Value>, AppError>;
    pub async fn delete_card(&self, Path(id): Path<String>) -> Result<Json<Value>, AppError>;
    pub async fn move_card(&self, Path(id): Path<String>, Json(body): Json<Value>) -> Result<Json<Value>, AppError>;
    pub async fn search_cards(&self, query: Query<String>) -> Result<Json<Value>, AppError>;
    pub async fn list_boards(&self) -> Json<Value>;
    pub async fn broadcast_event(&self, event: ServerSentEvent);
}
```

Each mutation handler follows the same pattern:
1. Open `Store` for `project_path`
2. Call the appropriate `store.*()` method
3. Call `broadcaster.send(event)` 
4. Return JSON response

### `src/webui/sse.rs`

```rust
pub struct ServerSentEvent {
    pub event: String,  // "card_created", "card_moved", etc.
    pub data: serde_json::Value,
}

pub async fn sse_handler(
    State(broadcaster): State<broadcast::Sender<ServerSentEvent>>,
) -> impl IntoResponse {
    let mut rx = broadcaster.subscribe();
    
    Streaming::new(async stream! {
        while let Ok(event) = rx.recv().await {
            yield Ok::<_, Infallible>(format!(
                "event: {}\ndata: {}\n\n",
                event.event,
                serde_json::to_string(&event.data).unwrap_or_default()
            ));
        }
    })
    .with_header("Content-Type", "text/event-stream")
    .with_header("Cache-Control", "no-cache")
    .with_header("Connection", "keep-alive")
}
```

---

## Dependencies

**New dependencies in `Cargo.toml` (feature-gated):**
```toml
axum = { version = "0.7", optional = true }
axum-server = { version = "0.7", features = ["tls-rustls"], optional = true }
tokio = { version = "1", features = ["sync"] }
tower-http = { version = "0.5", features = ["fs", "trace"], optional = true }
mime_guess = { version = "2", optional = true }
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

---

## File Served at Runtime

Frontend files are read from `webui/static/` at runtime (no embedding). During development, edit files and refresh the browser. During production (`cargo install`), the `webui/static/` directory is installed alongside the binary via a build script or `install` section.

---

## Self-Review Checklist

1. **Placeholder scan:** No TBD, TODO, or vague requirements. All endpoints specified with methods, paths, and I/O.
2. **Internal consistency:** API handlers call existing Store methods — no duplicate data logic. SSE broadcasts after every mutation. Frontend state matches TUI state (columns, cards, detail_card, focus).
3. **Scope check:** Focused single feature — WebUI companion to TUI. Not a full web app with auth, collaboration, or notifications. All TUI features included (full parity).
4. **Ambiguity check:** 
   - Port is explicit: 9876
   - SSE event types are explicit: card_created, card_moved, card_updated, card_deleted
   - Drag-and-drop uses HTML5 native API, no external library
   - Edit is inline (textarea + inputs), not external editor (though `e` could optionally spawn $EDITOR as TUI does)
   - Project switching fetches from existing `store.list_boards()` — reuses existing data
