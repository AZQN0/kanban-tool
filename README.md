# Kanban — A Kanban Board for Coding Agents

A Rust-based kanban board system designed for coding agents, with a CLI, a terminal UI (TUI), a WebUI, and an MCP server. Boards are stored as SQLite databases with markdown card files.

## Features

- **Multi-project boards** — Initialize separate kanban boards per project
- **Markdown-backed cards** — Each card is a human-readable markdown file with YAML frontmatter
- **SQLite persistence** — Fast, portable, zero-config database
- **Terminal UI (TUI)** — Full keyboard-driven TUI with 3-panel layout (columns / cards / detail)
- **WebUI** — Browser-based kanban board with REST API, SSE live updates, keyboard navigation, drag-and-drop, modals, and search
- **MCP Server** — 8 tools for programmatic card management (create, get, update, delete, list, move, search, manage)
- **CLI** — All operations available from the command line
- **Filters** — List cards by column, priority, or labels

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                      kanban                               │
├────────────┬────────────┬──────────────┬─────────────────┤
│    CLI     │    TUI     │    WebUI     │   MCP Server    │
│ (clap)     │ (ratatui)  │ (axum)       │ (rust-mcp-sdk)  │
├────────────┴────────────┴──────────────┴─────────────────┤
│                board/ (data layer)                        │
│   Store (SQLite) │ Card │ Column │ Label                  │
├───────────────────────────────────────────────────────────┤
│              kanban/ (project manager)                    │
│   Manager │ Config │ Init                                │
├───────────────────────────────────────────────────────────┤
│            markdown/ (persistence)                        │
│   Parser │ Writer                                        │
└───────────────────────────────────────────────────────────┘
```

## Installation

Requires Rust 1.75+.

```bash
git clone <repo>
cd kanban-tool
cargo build --release
```

For WebUI support, build with the `webui` feature:

```bash
cargo build --release --features webui
```

Or install via `cargo install`:

```bash
cargo install --path . --features webui
```

## Quick Start

```bash
# 1. Initialize a board
cd my-project && kanban init

# 2. Create some cards
kanban create --title "Fix login bug" --priority high
kanban create --title "Write tests" --priority medium --label backend

# 3. List everything
kanban list

# 4. Move a card
kanban move <CARD_ID> in_progress

# 5. Open the TUI (terminal UI)
kanban board

# 6. Open the WebUI (browser-based, requires --features webui)
kanban webui --port 8080

# 7. Start the MCP server (for AI coding tools)
kanban server
```

## CLI Reference

### `kanban init [PATH]`

Initialize a kanban board in the specified directory (defaults to `.`). Creates `.kanban/` with SQLite DB, `cards/` folder, and default columns: `backlog`, `todo`, `in_progress`, `review`, `done`.

### `kanban create [OPTIONS] --title <TITLE>`

Create a new card. **Required:** `--title`. Optional: `--description`, `--project`, `--column` (default: `todo`), `--priority` (default: `backlog`), `--label` (repeat for multiple).

```bash
kanban create --title "Implement login" \
  --description "POST /api/auth/login with JWT" \
  --priority high \
  --column todo \
  --label backend \
  --label security
```

### `kanban list [OPTIONS]`

List cards with optional filters.

```bash
kanban list                          # All cards
kanban list --priority high          # Filter by priority
kanban list --column in_progress     # Filter by column
kanban list --label security         # Filter by label
kanban list --project /path/to/proj  # Another project's board
```

**Priority values:** `backlog`, `low`, `medium`, `high`, `urgent`
**Column values:** `backlog`, `todo`, `in_progress`, `review`, `done`

### `kanban move <CARD_ID> <COLUMN>`

Move a card to a different column. Updates both the database and the markdown file.

### `kanban search [OPTIONS] <QUERY>`

Search across card titles and descriptions.

```bash
kanban search "login"
kanban search "auth" --project /path/to/project
```

### `kanban get <CARD_ID>`

Print full card details (title, column, priority, labels, description, timestamps).

### `kanban update <CARD_ID> [OPTIONS]`

Update any combination of card fields. All options are optional — only specify the fields you want to change.

```bash
kanban update <CARD_ID> --title "New title" --priority urgent --label backend
```

### `kanban delete <CARD_ID>`

Removes the card from both SQLite and the markdown file system.

### `kanban board`

Launch the terminal UI (TUI). Run from within an initialized project directory.

### `kanban webui [OPTIONS]`

Launch the WebUI in a browser. Requires build with `--features webui`.

```bash
kanban webui                     # Default: http://127.0.0.1:9876
kanban webui --port 8080         # Custom port
kanban webui --bind 0.0.0.0      # Bind all interfaces
kanban webui --bind 0.0.0.0 --port 3000
```

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `--bind` | `127.0.0.1` | Bind address |
| `--port` | `9876` | Port to listen on |

### `kanban server`

Start the MCP server for coding agents (runs over stdio).

## Terminal UI

Start with `kanban board` (run from within an initialized project).

### Layout

```
┌──────────────────────────────────────────────────────────┐
│ 📋 Project: my-project           Cards: 12/25            │
├──────────┬──────────────────────────┬────────────────────┤
│ ▶ backlog│ in_progress (3)          │ Fix auth token     │
│   todo (2)│ review (1)              │ Priority: high     │
│   done (0)│ done (2)                │ ────────────────── │
│           │                         │ When the access    │
│           │ • card-1                │ token expires...   │
│           │ ◉ card-2                │                    │
├──────────┴──────────────────────────┴────────────────────┤
│ ↑↓ Nav | Enter Focus | m Move | e Edit | D Delete | q Quit│
└──────────────────────────────────────────────────────────┘
```

### Key Bindings

| Key | Action |
|-----|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `h` / `←` | Focus previous panel |
| `l` / `→` | Focus next panel |
| `Enter` | Focus selected card in detail / Unfocus |
| `Esc` | Cancel mode / Unfocus detail |
| `m` | Move card (popup: `b`acklog, `t`odo, `i`n_progress, `r`eview, `d`one) |
| `e` | Open card in `$EDITOR` |
| `D` | Delete selected card |
| `P` | Switch project (project picker) |
| `/` | Start search |
| `q` | Quit |

## WebUI

Start with `kanban webui` (requires `--features webui`). Opens a browser-based kanban board with live updates via Server-Sent Events.

### Layout

```
┌──────────────────────────────────────────────────────────┐
│ 📋 Project Name          Cards: 12   Focus: Cards  Ready │
├──────────┬───────────────┬───────────────────────────────┤
│ backlog  │ in_progress   │ Card Title                      │
│ todo (2) │ review        │ Priority: high                  │
│ done     │ done          │ ──────────────────              │
│          │               │ Description...                  │
│          │ • Card 1      │ Labels: [backend, critical]     │
│          │ • Card 2      │ Created: 2026-05-19             │
│          │ ◉ Card 3      │ Updated: 2026-05-19             │
├──────────┴───────────────┴────────────────────────────────┤
│ Navigate | Focus | Move(m) | Delete(D) | Edit(e) | Search(/)│
└───────────────────────────────────────────────────────────┘
```

### WebUI Features

- **Live updates** — Changes from other clients appear in real-time via SSE
- **Keyboard navigation** — Full keyboard-driven interaction (move `m`, delete `D`, edit `e`, search `/`)
- **Modal dialogs** — Move, delete, and edit operations via popups
- **Search** — Press `/` to search cards by title/description
- **Responsive columns** — Click any column to view its cards

### WebUI Key Bindings

| Key | Action |
|-----|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `h` / `←` | Focus previous panel |
| `l` / `→` | Focus next panel |
| `m` | Open move modal |
| `D` | Open delete modal |
| `e` | Edit card in `$EDITOR` |
| `/` | Open search input |
| `Escape` | Close modal / Cancel |

### WebUI REST API

When running, the WebUI exposes a REST API:

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/boards` | List all boards |
| `GET` | `/api/cards` | Get all cards grouped by column |
| `GET` | `/api/cards/:id` | Get a single card |
| `POST` | `/api/cards` | Create a new card |
| `PATCH` | `/api/cards/:id` | Update card fields |
| `DELETE` | `/api/cards/:id` | Delete a card |
| `POST` | `/api/cards/:id/move` | Move card to column |
| `GET` | `/api/cards/search?q=term` | Search cards |
| `GET` | `/api/events` | SSE feed for live updates |

## MCP Server

Start with `kanban server` (runs over stdio). Exposes 8 tools:

| Tool | Required Args | Description |
|------|---------------|-------------|
| `create_card` | `project`, `title` | Create a new card |
| `get_card` | `card_id` | Get full card data |
| `update_card` | `card_id` | Update card fields |
| `delete_card` | `card_id` | Delete a card |
| `list_cards` | — | List cards (all optional filters) |
| `transition_card` | `card_id`, `column` | Move card to column |
| `search_cards` | `query` | Search title/description |
| `manage_board` | `action` | Init board, add/remove columns |

### Example: Create via MCP

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
  "name":"create_card",
  "arguments":{"project":"/path/to/proj","title":"New feature","priority":"medium"}
}}
```

## Data Model

### Card File

Each card is stored as `.kanban/cards/<id>.md`:

```markdown
---
id: <uuid>
board_id: <uuid>
column_id: <uuid>
title: Card title
priority: low|medium|high|urgent|backlog
labels: []
subtasks: []
parent_card_id: null
created_at: 2026-05-19T...
updated_at: 2026-05-19T...
---

Card description body (markdown)
```

### Project Structure

```
my-project/
├── .kanban/
│   ├── kanban.db          # SQLite database (tables: boards, columns, cards, comments)
│   ├── cards/             # Markdown card files (one per card)
│   └── columns/           # Column definitions
```

## AI Agent Skill

This project ships with an AI agent skill (`skills/kanban/SKILL.md`) covering all CLI commands, MCP tool usage, common workflows, and the card file format. Point your agent's skill directory at `skills/` in this repo.

## Testing

### Unit & Integration Tests

```bash
cargo test              # Run all tests (21 pass)
cargo clippy --all-features  # Linting (clean build, no warnings)
```

### E2E Tests (WebUI)

Requires Playwright and a Chromium browser:

```bash
# Install dependencies
npm install
npx playwright install chromium

# Run all E2E tests
bash e2e/run.sh
# or
npm run e2e
```

12 E2E tests cover: page load, columns, cards, detail view, move/delete modals, keyboard navigation, search, messages, and empty state.

## Development

```bash
cargo build
cargo test
cargo clippy
```

## ⚠️ Disclaimer

> This project was **entirely vibecoded with [Qwen3.6-35B-A3B-GGUF](https://huggingface.co/Qwen/Qwen3.6-35B-A3B-GGUF)**. The code works, but use at your own risk.
