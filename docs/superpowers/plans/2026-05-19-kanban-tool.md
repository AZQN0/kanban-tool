# Kanban Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust kanban board tool with CLI, TUI, and MCP interfaces for coding agents across multiple projects.

**Architecture:** Per-project boards stored in `.kanban/` with SQLite as source of truth and markdown files as human-readable parallel representation. Three interfaces share a common board domain layer.

**Tech Stack:** Rust 1.95, clap (derive), rusqlite, ratatui, crossterm, mcp (Rust SDK), serde, chrono, pulldown-cmark.

---

### Task 1: Project Setup & Cargo.toml

**Goal:** Create the Rust workspace with all dependencies and module structure.

**Files:**
- Create: `Cargo.toml` — binary crate, all dependencies listed
- Create: `src/main.rs` — entry point with clap subcommands
- Create: `src/board/mod.rs`, `src/kanban/mod.rs`, `src/markdown/mod.rs`, `src/mcp/mod.rs`, `src/cli/mod.rs`, `src/tui/mod.rs`
- Create: `src/board/store.rs`, `src/board/card.rs`, `src/board/column.rs`, `src/board/label.rs`
- Create: `src/markdown/writer.rs`, `src/markdown/parser.rs`
- Create: `src/kanban/manager.rs`, `src/kanban/init.rs`, `src/kanban/config.rs`
- Create: `src/mcp/server.rs`, `src/mcp/handlers.rs`, `src/mcp/schema.rs`
- Create: `src/cli/list.rs`, `src/cli/create.rs`, `src/cli/transition.rs`, `src/cli/search.rs`
- Create: `src/tui/app.rs`, `src/tui/render.rs`, `src/tui/events.rs`
- Create: `migrations/001_init.sql`
- Create: `tests/integration_test.rs`

**Design:**

**Cargo.toml dependencies:**
- `clap = { version = "4", features = ["derive"] }` — CLI parsing
- `rusqlite = { version = "0.32", features = ["bundled"] }` — SQLite
- `serde = { version = "1", features = ["derive"] }` — serialization
- `serde_json = "1"` — JSON handling
- `chrono = { version = "0.4", features = ["serde"] }` — timestamps
- `uuid = { version = "1", features = ["v4", "serde"] }` — card IDs
- `ratatui = "0.29"` — TUI rendering
- `crossterm = "0.28"` — terminal input
- `pulldown-cmark = "0.11"` — markdown parsing
- `mcp = "0.3"` — MCP protocol server
- `rusqlite_migration = "1"` — schema migrations
- `anyhow = "1"` — error handling
- `console = "0.15"` — terminal table formatting

**Module structure (all empty `mod.rs` files with `pub mod` declarations):**

```
src/
├── main.rs            # CLI + TUI + MCP dispatch
├── board/             # Data layer: SQLite + card/column models
│   ├── mod.rs
│   ├── store.rs       # Database connection + queries
│   ├── card.rs        # Card struct + CRUD operations
│   ├── column.rs      # Column struct + default definitions
│   └── label.rs       # Label management
├── kanban/            # Project-level board management
│   ├── mod.rs
│   ├── manager.rs     # Open/close/find board for a project path
│   ├── init.rs        # Initialize .kanban/ directory and database
│   └── config.rs      # Board configuration struct
├── markdown/          # Markdown card files (human-readable)
│   ├── mod.rs
│   ├── writer.rs      # Write card to .md file
│   └── parser.rs      # Parse card from .md file
├── mcp/               # MCP server for agent tool access
│   ├── mod.rs
│   ├── server.rs      # MCP server lifecycle
│   ├── handlers.rs    # Tool call implementations
│   └── schema.rs      # JSON Schema definitions for each tool
├── cli/               # Human CLI commands
│   ├── mod.rs
│   ├── list.rs        # kanban list
│   ├── create.rs      # kanban create
│   ├── transition.rs  # kanban move
│   └── search.rs      # kanban search
└── tui/               # Terminal UI
    ├── mod.rs
    ├── app.rs         # State machine: board view, card detail, editing
    ├── render.rs      # Ratatui widget rendering
    └── events.rs      # Key event handling
```

**Key types (defined in `src/board/card.rs`):**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: String,              // UUID v4
    pub board_id: String,        // UUID of the board
    pub column_id: String,       // UUID of the column
    pub title: String,
    pub description: String,     // Markdown body
    pub priority: Priority,
    pub labels: Vec<String>,
    pub subtasks: Vec<String>,   // Card IDs of subtask cards
    pub parent_card_id: Option<String>,
    pub card_file: String,       // Relative path to markdown file
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Priority {
    Backlog,
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub sort_order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: String,
    pub project_path: String,
    pub name: String,
    pub columns: Vec<Column>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub card_id: String,
    pub author: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}
```

**Key types (defined in `src/kanban/config.rs`):**

```rust
pub const DEFAULT_COLUMNS: &[&str] = &["backlog", "todo", "in_progress", "review", "done"];
pub const KANBAN_DIR: &str = ".kanban";
pub const DATABASE_NAME: &str = "kanban.db";
pub const CARDS_DIR: &str = "cards";

pub struct BoardConfig {
    pub columns: Vec<Column>,
}

**Error handling:** All functions return `anyhow::Result<T>` with `.context("action")` for meaningful error messages. Custom error types not needed — anyhow is sufficient for this scope.
```

**Default columns (from `src/board/column.rs`):**

```rust
impl Column {
    pub fn default_columns(board_id: &str) -> Vec<Self> {
        DEFAULT_COLUMNS.iter().enumerate().map(|(i, name)| {
            Self {
                id: uuid::Uuid::new_v4().to_string(),
                board_id: board_id.to_string(),
                name: name.to_string(),
                sort_order: i as u32,
            }
        }).collect()
    }
}
```

**Key behaviors to test:**
1. All types serialize/deserialize correctly via serde
2. Priority enum round-trips through JSON
3. Default columns are created in the correct order (backlog=0, todo=1, ..., done=4)

---

### Task 2: SQLite Data Layer

**Goal:** Implement the SQLite storage backend with schema migrations, connection management, and all CRUD operations for cards, columns, and comments.

**Files:**
- Modify: `migrations/001_init.sql` — full schema
- Create: `src/board/store.rs` — database connection, migrations, all queries
- Modify: `src/board/card.rs` — add database-backed CRUD methods
- Modify: `src/board/column.rs` — add database-backed CRUD methods
- Modify: `src/board/label.rs` — label query methods

**SQL Schema (from `migrations/001_init.sql`):**

```sql
CREATE TABLE IF NOT EXISTS boards (
    id TEXT PRIMARY KEY,
    project_path TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS columns (
    id TEXT PRIMARY KEY,
    board_id TEXT NOT NULL REFERENCES boards(id),
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS cards (
    id TEXT PRIMARY KEY,
    board_id TEXT NOT NULL REFERENCES boards(id),
    column_id TEXT NOT NULL REFERENCES columns(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    priority TEXT NOT NULL DEFAULT 'backlog',
    labels TEXT NOT NULL DEFAULT '[]',
    subtasks TEXT NOT NULL DEFAULT '[]',
    parent_card_id TEXT,
    card_file TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (parent_card_id) REFERENCES cards(id)
);

CREATE TABLE IF NOT EXISTS comments (
    id TEXT PRIMARY KEY,
    card_id TEXT NOT NULL REFERENCES cards(id),
    author TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_cards_board ON cards(board_id);
CREATE INDEX IF NOT EXISTS idx_cards_column ON cards(column_id);
CREATE INDEX IF NOT EXISTS idx_comments_card ON comments(card_id);
CREATE INDEX IF NOT EXISTS idx_cards_priority ON cards(priority);
CREATE INDEX IF NOT EXISTS idx_cards_labels ON cards(labels);
```

**Database interface (`src/board/store.rs`):**

```rust
use anyhow::Result;
use rusqlite::{Connection, params};
use rusqlite_migration::Migrations;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &str) -> Result<Self>;
    pub fn apply_migrations(&mut self) -> Result<()>;
    pub fn create_board(id: &str, project_path: &str, name: &str) -> Result<()>;
    pub fn get_board(&self, project_path: &str) -> Result<Board>;
    pub fn add_column(&self, board_id: &str, column: &Column) -> Result<()>;
    pub fn get_columns(&self, board_id: &str) -> Result<Vec<Column>>;
    pub fn create_card(&self, card: &Card) -> Result<()>;
    pub fn get_card(&self, card_id: &str) -> Result<Card>;
    pub fn update_card(&self, card_id: &str, title: Option<&str>, description: Option<&str>,
                       column_id: Option<&str>, priority: Option<&str>, labels: Option<&[String]>) -> Result<()>;
    pub fn delete_card(&self, card_id: &str) -> Result<()>;
    pub fn list_cards(&self, board_id: &str, column_id: Option<&str>, priority: Option<&str>,
                      labels: Option<&[String]>, sort_by: &str) -> Result<Vec<Card>>;
    pub fn search_cards(&self, board_id: &str, query: &str) -> Result<Vec<Card>>;
    pub fn transition_card(&self, card_id: &str, new_column_id: &str) -> Result<()>;
    pub fn add_comment(&self, card_id: &str, author: &str, content: &str) -> Result<()>;
    pub fn get_comments(&self, card_id: &str) -> Result<Vec<Comment>>;
    pub fn find_cards_by_parent(&self, parent_id: &str) -> Result<Vec<Card>>;
    pub fn update_subtasks(&self, card_id: &str, subtask_ids: &[String]) -> Result<()>;
}
```

**Search implementation note:** Use SQLite `LIKE` or `MATCH` (FTS5 extension) on `cards.title || ' ' || cards.description`. FTS5 gives better performance for large boards. The `search_cards` method builds a query like:
```sql
SELECT * FROM cards WHERE (title LIKE '%query%' OR description LIKE '%query%') AND board_id = ?
```

**Key behaviors to test:**
1. Creating a board with default columns creates 5 rows in `columns` table
2. Creating a card writes to SQLite and returns success
3. Updating a card's description changes it in the database
4. Transitioning a card to a new column updates `column_id`
5. Searching returns cards matching the query in title or description
6. Deleting a card removes it from the `cards` table
7. Creating a comment links it to the correct card

---

### Task 3: Markdown Card Writer/Parser

**Goal:** Implement reading and writing card data to markdown files in `.kanban/cards/`, with YAML frontmatter for metadata.

**Files:**
- Create: `src/markdown/parser.rs` — parse frontmatter + body from .md
- Create: `src/markdown/writer.rs` — write card struct to .md file

**File naming convention (from `src/markdown/writer.rs`):**

Card files are named `{priority}-{slug}.md` where `slug` is the title lowercased with spaces replaced by hyphens:

```rust
pub fn card_filename(priority: &str, title: &str) -> String {
    let slug = title.to_lowercase()
        .replace(' ', "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "");
    format!("{}.md", format!("{}-{}", priority, slug))
}
```

**Frontmatter format (what parser.rs must produce):**

```rust
pub struct CardMetadata {
    pub id: String,
    pub board: String,
    pub column: String,
    pub priority: String,
    pub labels: Vec<String>,
    pub created: String,
    pub updated: String,
}

pub struct ParsedCard {
    pub metadata: CardMetadata,
    pub body: String,  // Markdown content after frontmatter
}
```

**Parser implementation (`src/markdown/parser.rs`):**

```rust
use chrono::Utc;

pub fn parse_card(content: &str) -> ParsedCard;
pub fn parse_frontmatter(content: &str) -> CardMetadata;
```

The parser extracts YAML frontmatter (between `---` delimiters) and the remaining body. Use a simple approach: find the first `---` line and the second `---` line, split on those boundaries, then parse the YAML section manually (key: value pairs, with arrays as `["a", "b"]`). No external YAML dependency needed for this simple format.

**Writer implementation (`src/markdown/writer.rs`):**

```rust
pub fn write_card(card: &Card, board_name: &str, path: &str) -> Result<String>;
```

This writes to `{path}/.kanban/cards/{filename}`:

```yaml
---
id: "{card.id}"
board: "{board_name}"
column: "{column_name}"
priority: "{card.priority}"
labels: {json_array}
created: "{card.created_at.to_rfc3339()}"
updated: "{card.updated_at.to_rfc3339()}"
---

{card.description}
```

The function returns the full file path written.

**Key behaviors to test:**
1. Writing a card produces a valid .md file with frontmatter + body
2. Parsing that file back produces the same metadata and body
3. Card filenames are deterministic (same title → same filename)
4. Special characters in titles are handled in filenames (slashes, quotes)

---

### Task 4: Project Board Manager

**Goal:** Implement the `kanban/` module that manages opening, initializing, and locating boards for project paths.

**Files:**
- Create: `src/kanban/manager.rs` — open/find board for a project
- Create: `src/kanban/init.rs` — create a new board for a project
- Modify: `src/kanban/config.rs` — add board configuration helpers

**Manager interface (`src/kanban/manager.rs`):**

```rust
use std::path::Path;
use anyhow::Result;

pub struct BoardManager {
    store: Store,
}

impl BoardManager {
    pub fn new(db_path: &Path) -> Result<Self>;
    pub fn find_board(&self, project_path: &Path) -> Result<Option<Board>>;
    pub fn open_board(&self, project_path: &Path) -> Result<Board>;
    pub fn list_all_projects(&self) -> Result<Vec<Board>>;
    pub fn get_project_path(&self, board_id: &str) -> Result<String>;
}
```

**Init interface (`src/kanban/init.rs`):**

```rust
pub fn init_board(project_path: &Path) -> Result<Board>;
```

This does:
1. Create `.kanban/` directory
2. Create `.kanban/cards/` subdirectory
3. Open SQLite at `.kanban/kanban.db`
4. Apply migrations
5. Create board row (name = project directory name)
6. Create default columns (backlog, todo, in_progress, review, done)
7. Return the Board struct

**Key behaviors to test:**
1. `init_board` creates the `.kanban/` directory structure and database
2. `find_board` returns `Some(board)` for a project with `.kanban/`, `None` otherwise
3. `list_all_projects` returns all initialized boards
4. Opening a board from a different path than originally initialized still works (uses project_path column)

---

### Task 5: CLI Interface

**Goal:** Implement all human-facing CLI commands using clap subcommands.

**Files:**
- Create: `src/main.rs` — entry point with clap derive, dispatches to subcommands
- Create: `src/cli/list.rs` — `kanban list`
- Create: `src/cli/create.rs` — `kanban create`
- Create: `src/cli/transition.rs` — `kanban move`
- Create: `src/cli/search.rs` — `kanban search`

**Entry point (`src/main.rs`):**

```rust
#[derive(clap::Args, Debug)]
struct ListArgs {
    #[arg(long)]
    project: Option<String>,
    #[arg(long, value_parser = ["backlog", "todo", "in_progress", "review", "done"])]
    column: Option<String>,
    #[arg(long)]
    label: Option<Vec<String>>,
    #[arg(long, value_parser = ["backlog", "low", "medium", "high", "urgent"])]
    priority: Option<String>,
}

#[derive(clap::Args, Debug)]
struct CreateArgs {
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    column: Option<String>,
    #[arg(long, default_value = "backlog")]
    priority: String,
    #[arg(long)]
    label: Option<Vec<String>>,
}

#[derive(clap::Args, Debug)]
struct MoveArgs {
    card_id: String,
    column: String,
}

#[derive(clap::Args, Debug)]
struct SearchArgs {
    query: String,
    #[arg(long)]
    project: Option<String>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    Init { path: String },
    List(ListArgs),
    Create(CreateArgs),
    Move(MoveArgs),
    Search(SearchArgs),
    Board,   // TUI
    Server,  // MCP server
}
```

**Command dispatch:**

- `kanban init <path>` → `kanban::init::init_board()`
- `kanban list` → `store.list_cards()` with filters, print table to stdout
- `kanban create` → find project board → `store.create_card()` + `markdown::writer::write_card()`
- `kanban move <id> <column>` → find column by name → `store.transition_card()` + update markdown file
- `kanban search <query>` → `store.search_cards()` with optional project filter, print results

**Output format (list/search):**

Print a compact table to stdout:
```
ID          TITLE                              COLUMN        PRIORITY
card-001    Fix auth token refresh             in_progress   high
card-002    Add user settings                  todo            medium
card-003    Research MCP protocols             backlog         low
```

Use `crossterm` or `console` crate for terminal table formatting. Keep it simple: pipe-delimited or aligned columns.

**Key behaviors to test:**
1. `init` creates a board at the specified path
2. `create` adds a card to the specified project's board
3. `move` transitions a card to the correct column
4. `list` with `--column in_progress` only shows in_progress cards
5. `list` with `--label bug` only shows cards with the "bug" label
6. `search` finds cards by title and description text

---

### Task 6: MCP Server

**Goal:** Implement the MCP server that exposes 8 tools over stdio transport for agent programmatic access.

**Files:**
- Create: `src/mcp/server.rs` — MCP server lifecycle, stdio transport
- Create: `src/mcp/handlers.rs` — tool call implementations
- Create: `src/mcp/schema.rs` — JSON Schema for each tool's parameters and results

**Server interface (`src/mcp/server.rs`):**

```rust
use mcp::server::Server;
use mcp::transport::stdio::StdioTransport;

pub async fn run_server() -> Result<()>;
```

The server:
1. Creates an MCP server with name "kanban" and version "0.1.0"
2. Registers all 8 tools with their schemas
3. Runs on stdio transport (LSP-style message passing)
4. Each `tools/call` request is dispatched to the appropriate handler in `handlers.rs`

**Tool schemas (`src/mcp/schema.rs`):**

```rust
// create_card parameters JSON Schema
pub const CREATE_CARD_SCHEMA: &str = r#"
{
  "type": "object",
  "required": ["project", "title"],
  "properties": {
    "project": { "type": "string", "description": "Path to the project directory" },
    "title": { "type": "string", "description": "Card title" },
    "description": { "type": "string", "description": "Card description in markdown" },
    "column": { "type": "string", "enum": ["backlog", "todo", "in_progress", "review", "done"] },
    "priority": { "type": "string", "enum": ["backlog", "low", "medium", "high", "urgent"] },
    "labels": { "type": "array", "items": { "type": "string" } }
  }
}
"#;

// Similar schemas for each tool...
pub const GET_CARD_SCHEMA: &str = r#"
{
  "type": "object",
  "required": ["card_id"],
  "properties": {
    "card_id": { "type": "string" }
  }
}
"#;

pub const UPDATE_CARD_SCHEMA: &str = r#"
{
  "type": "object",
  "required": ["card_id"],
  "properties": {
    "card_id": { "type": "string" },
    "title": { "type": "string" },
    "description": { "type": "string" },
    "column": { "type": "string" },
    "priority": { "type": "string" },
    "labels": { "type": "array", "items": { "type": "string" } }
  }
}
"#;

pub const DELETE_CARD_SCHEMA: &str = r#"
{
  "type": "object",
  "required": ["card_id"],
  "properties": {
    "card_id": { "type": "string" }
  }
}
"#;

pub const LIST_CARDS_SCHEMA: &str = r#"
{
  "type": "object",
  "properties": {
    "project": { "type": "string" },
    "column": { "type": "string" },
    "priority": { "type": "string" },
    "labels": { "type": "array", "items": { "type": "string" } },
    "limit": { "type": "integer", "default": 50 },
    "offset": { "type": "integer", "default": 0 }
  }
}
"#;

pub const TRANSITION_CARD_SCHEMA: &str = r#"
{
  "type": "object",
  "required": ["card_id", "column"],
  "properties": {
    "card_id": { "type": "string" },
    "column": { "type": "string" }
  }
}
"#;

pub const SEARCH_CARDS_SCHEMA: &str = r#"
{
  "type": "object",
  "required": ["query"],
  "properties": {
    "query": { "type": "string" },
    "project": { "type": "string" }
  }
}
"#;

pub const MANAGE_BOARD_SCHEMA: &str = r#"
{
  "type": "object",
  "required": ["action"],
  "properties": {
    "action": { "type": "string", "enum": ["init", "add_column", "remove_column"] },
    "project": { "type": "string" },
    "column_name": { "type": "string" }
  }
}
"#;
```

**Handler interface (`src/mcp/handlers.rs`):**

```rust
use crate::board::store::Store;
use crate::kanban::manager::BoardManager;
use serde_json::Value;

pub struct Handlers {
    manager: BoardManager,
}

impl Handlers {
    pub fn new(manager: BoardManager) -> Self;
    pub fn handle_create_card(&self, args: &Value) -> Result<String>;
    pub fn handle_get_card(&self, args: &Value) -> anyhow::Result<String>;
    pub fn handle_update_card(&self, args: &Value) -> anyhow::Result<String>;
    pub fn handle_delete_card(&self, args: &Value) -> anyhow::Result<String>;
    pub fn handle_list_cards(&self, args: &Value) -> anyhow::Result<String>;
    pub fn handle_transition_card(&self, args: &Value) -> anyhow::Result<String>;
    pub fn handle_search_cards(&self, args: &Value) -> anyhow::Result<String>;
    pub fn handle_manage_board(&self, args: &Value) -> anyhow::Result<String>;
}
```

Each handler:
1. Parses JSON args into typed parameters
2. Finds the board for the project (returns error if not initialized)
3. Performs the operation via `Store` + `MarkdownWriter`
4. Returns the result as a JSON string

**Server dispatch (`src/mcp/server.rs`):**

The server registers tools with the MCP framework. Each `tools/call` request includes a `name` field matching the tool name and an `arguments` object. The server routes to the correct handler based on the tool name.

**Key behaviors to test:**
1. `tools/list` returns 8 tools with correct names and schemas
2. `tools/call` with `create_card` creates a card and returns its ID
3. `tools/call` with `get_card` returns the card including description
4. `tools/call` with `list_cards` filters correctly by column and priority
5. `tools/call` with `search_cards` returns matching cards
6. Calling a tool on a non-initialized project returns an error

---

### Task 7: TUI Interface

**Goal:** Implement the ratatui terminal UI with a board view, card detail pane, and keyboard navigation.

**Files:**
- Create: `src/tui/app.rs` — state machine
- Create: `src/tui/render.rs` — ratatui rendering
- Create: `src/tui/events.rs` — key event handling
- Modify: `src/main.rs` — add `tui::app::run()` entry

**Application state (`src/tui/app.rs`):**

```rust
pub enum Tab {
    Board,      // Main board view
    Cards,      // Card list (filtered)
    Search,     // Active search
}

pub enum Focus {
    Columns,    // Left panel: columns
    Cards,      // Center panel: cards list
    Detail,     // Right panel: card detail
}

pub struct App {
    pub running: bool,
    pub tab: Tab,
    pub focus: Focus,
    pub board: Board,
    pub cards: Vec<Card>,
    pub selected_card_idx: usize,
    pub selected_detail: Option<Card>,
    pub editing: bool,
    pub search_query: String,
    pub message: Option<String>,
    pub project_index: usize,
    pub all_projects: Vec<Board>,
}
```

**Rendering (`src/tui/render.rs`):**

```rust
use ratatui::{Frame, Layout, Direction};
use ratatui::widgets::{Block, Borders, Tabs, List, ListItem, Paragraph, Widget};

pub fn render(frame: &mut Frame, app: &mut App);
fn render_columns(frame: &mut Frame, app: &App, area: ratatui::layout::Rect);
fn render_cards(frame: &mut Frame, app: &App, area: ratatui::layout::Rect);
fn render_detail(frame: &mut Frame, app: &App, area: ratatui::layout::Rect);
fn render_statusbar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect);
```

**Layout:**

```
┌─────────────────────────────────────────────────────────┐
│ Project: my-project                    Cards: 12/25      │  ← Status bar
├──────────┬──────────────────────────┬───────────────────┤
│ backlog  │ Todo (3)                 │ Fix auth token    │
│ ┌──────┐ │ ┌─────────────────────┐  │ refresh           │
│ │card-1│ │ │ card-2              │  │ ───────────────── │
│ │card-3│ │ │ card-4              │  │ In Progress       │
│ └──────┘ │ │ card-5              │  │ • Check token     │
│ in_prog  │ │                     │  │ • If expired...   │
│ ┌──────┐ │ │ <card-6>            │  │ ───────────────── │
│ │card-7│ │ │                     │  │ Progress          │
│ └──────┘ │ │                     │  │ - [x] Identified  │
│          │ │                     │  │ - [ ] Implement   │
│ review   │ │                     │  │                   │
│ ┌──────┐ │ │                     │  │                   │
│ │card-8│ │ │                     │  │                   │
│ └──────┘ │ │                     │  │                   │
│          │ │                     │  │                   │
│ done     │ │                     │  │                   │
│ ┌──────┐ │ │                     │  │                   │
│ │card-9│ │ │                     │  │                   │
│ └──────┘ │ │                     │  │                   │
├──────────┴────────────────────────┴───────────────────┤
│ ↑↓ Navigate  Enter Focus  m Move  e Edit  d Delete  P Project  │  ← Status bar
└─────────────────────────────────────────────────────────┘
```

**Event handling (`src/tui/events.rs`):**

```rust
pub fn handle_key(event: Key, app: &mut App) -> anyhow::Result<()>;
```

Key bindings:
- `j` / `Down` — Move selection down one card
- `k` / `Up` — Move selection up one card
- `h` / `Left` — Shift focus to previous panel
- `l` / `Right` — Shift focus to next panel
- `Enter` — Focus selected card in detail pane
- `Esc` — Unfocus detail pane / deselect card
- `m` — Move selected card: show column selector popup
- `e` — Open card in `$EDITOR` (spawn `env EDITOR` as subprocess)
- `d` — Delete selected card (confirm with `y`/`n`)
- `P` — Switch project: show project list picker
- `/` — Start search: show search input
- `q` — Quit

**Move popup:** When pressing `m`, display a small popup listing all 5 columns. User presses the column letter to confirm the move.

**Search:** When pressing `/`, show an input line. On Enter, filter cards matching the query. Press `Esc` to exit search mode.

**Key behaviors to test:**
1. Board renders with all columns and their cards
2. Arrow keys navigate between cards within a column
3. Focus moves between panels (columns → cards → detail)
4. Pressing `e` opens the card's markdown file in $EDITOR
5. Pressing `m` shows a column selector popup
6. Pressing `P` shows a project picker for multi-project boards

---

## Execution Order

Tasks should be implemented in order: 1 → 2 → 3 → 4 → 5 → 6 → 7. Each task builds on the previous ones. After tasks 1-4, the core data system works (CLI `init` + `list` + `create` + `move` are functional). Task 5 completes the CLI. Task 6 adds MCP. Task 7 adds the TUI.
