# Kanban Tool Design

A kanban board system designed as infrastructure for coding agents across multiple projects. Three interfaces: CLI for humans, TUI for browsing, MCP for agent programmatic access.

## Architecture

```
kanban-tool/
├── src/
│   ├── main.rs              # CLI entry point + subcommands
│   ├── mcp/
│   │   ├── server.rs        # MCP server implementation
│   │   ├── handlers.rs      # Tool handler implementations
│   │   └── schema.rs        # JSON Schema definitions for MCP tools
│   ├── board/
│   │   ├── store.rs         # SQLite board storage layer
│   │   ├── card.rs          # Card model + operations
│   │   ├── column.rs        # Column model + transitions
│   │   └── label.rs         # Labels/tags system
│   ├── kanban/
│   │   ├── manager.rs       # Multi-project board management
│   │   ├── init.rs          # Project initialization
│   │   └── config.rs        # Board configuration
│   ├── tui/
│   │   ├── app.rs           # TUI state machine
│   │   ├── render.rs        # Terminal rendering
│   │   └── events.rs        # Input handling
│   ├── cli/
│   │   ├── list.rs          # List cards command
│   │   ├── create.rs        # Create card command
│   │   ├── transition.rs    # Move cards between columns
│   │   └── search.rs        # Search/filter cards
│   └── markdown/
│       ├── writer.rs        # Write card.md files to disk
│       └── parser.rs        # Parse card metadata from frontmatter
├── migrations/
│   └── 001_init.sql         # SQLite schema migration
├── tests/                   # Integration tests
├── Cargo.toml
└── kanban.toml              # Global config (optional)
```

## Data Layer

### SQLite Tables (source of truth)

- **boards** — id, project_path, name, created_at, updated_at
- **columns** — id, board_id, name, sort_order (backlog, todo, in_progress, review, done)
- **cards** — id, board_id, column_id, title, description, priority, labels (JSON array), subtasks (JSON), parent_card_id (nullable), created_at, updated_at, card_file (path to markdown)
- **comments** — id, card_id, author, content, created_at

### Markdown Cards (human-readable, git-trackable)

Card markdown files live in the project's `.kanban/` directory as `{priority}-{title}.md`:

```yaml
---
id: "card-001"
board: "my-project"
column: "in_progress"
priority: high
labels: ["bug", "auth"]
created: "2026-05-19T10:00:00Z"
updated: "2026-05-19T14:30:00Z"
---

# Fix auth token refresh

## Description
When the access token expires, the client should silently refresh...

## Plan
1. Check token expiry before API call
2. If expired, call /refresh endpoint
3. Retry original request

## Progress
- [x] Identified the issue in token_handler.rs
- [ ] Implement refresh logic
```

**Key principle:** SQLite is the authoritative store. Markdown files are a parallel, human-readable representation. All writes update both in sync. Reads can come from either source, but the CLI and MCP always go through SQLite to ensure consistency.

### Storage Locations

- **Per-project board:** `.kanban/kanban.db` (SQLite) + `.kanban/cards/*.md` (markdown cards)

The `.kanban/` directory sits at the project root (git-tracked), containing both the SQLite database and markdown card files. No global database — each project owns its own board.

## MCP Tools (8 tools)

| Tool | Description |
|------|-------------|
| `create_card` | Create a new card. Accepts title, description (markdown), column (default: backlog), priority, labels. Returns card ID. |
| `get_card` | Read a card by ID. Returns full content including markdown body. |
| `update_card` | Update card fields: title, description, column, priority, labels. Partial updates supported. |
| `delete_card` | Delete a card permanently. |
| `list_cards` | List cards with filters: by column, priority, labels, project. Supports pagination. |
| `transition_card` | Move a card to a different column. Any column transition is allowed. |
| `search_cards` | Full-text search across card titles, descriptions, and comments. |
| `manage_board` | Initialize a new board for a project, add/remove columns, reconfigure board settings. |

### MCP Protocol

The server exposes tools over stdio using the MCP `tools/list` → `tools/call` protocol. Each tool returns structured JSON — never markdown text. Agents discover tools via standard MCP discovery.

## CLI Interface

```bash
kanban init <project-path>          # Create .kanban/ directory with board in project
kanban list [--project PATH] [--column STATUS] [--label TAG] [--priority LEVEL]
kanban create --title "..." [--project PATH] [--priority high] [--label "bug"]
kanban move <card-id> <column>      # Transition card between columns
kanban view <card-id>               # Open card markdown in $EDITOR
kanban search <query>               # Search across all known projects
kanban board                        # TUI mode
kanban server                       # Start MCP server (stdio transport)
```

## TUI Design

Full-screen terminal UI using `ratatui`:

- **Left panel:** Board columns (backlog → todo → in_progress → review → done) showing cards as compact rows with priority indicators
- **Center detail:** Selected card's markdown preview in a scrollable pane
- **Bottom bar:** Status line showing project name, card count, current focus, keyboard shortcuts
- **Navigation:** Arrow keys move between cards, Enter focuses, Esc deselects
- **Editing:** Press `e` to open card in `$EDITOR`, `m` to move to another column, `d` to delete
- **Project switching:** `P` to switch between projects

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` (with `derive`) | CLI argument parsing |
| `rusqlite` + `rusqlite_migration` | SQLite storage and schema migrations |
| `serde` + `serde_json` | Serialization |
| `pulldown-cmark` | Markdown parsing for TUI preview |
| `ratatui` + `crossterm` | Terminal UI rendering |
| `mcp` | MCP protocol server (Rust SDK) |
| `chrono` | Timestamps with timezone support |
| `globset` | Project path matching for multi-project search |

## Key Design Decisions

1. **One board per project** — The `.kanban/` directory at project root contains both the SQLite database and markdown card files. Everything is local to the project, git-trackable, and self-contained.

2. **Free-form transitions** — Cards can move between any columns. No validation or required flow. The TUI shows the columns left-to-right in the standard kanban order, but agents can move cards anywhere via CLI or MCP.

3. **Priority levels** — backlog, low, medium, high, urgent. Cards sort within columns by priority (urgent first).

4. **Unique card IDs** — Every card gets a UUID. No deduplication logic. MCP reads and updates are idempotent when given a card ID.

5. **Labels** — Free-form string tags. Multiple labels per card. Filterable in list/search operations.

6. **Subtasks** — Hierarchical card support. A parent card can have subtask cards referenced via `parent_card_id`. Displayed as collapsible sections in the TUI.

7. **Comments are agent-only** — Time-stamped additions stored in SQLite. Not written to markdown files. Markdown is for human-readable card content; comments are for agent conversation history on a card.
