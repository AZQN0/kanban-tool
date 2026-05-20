# Kanban — A Kanban Board for Coding Agents

A Rust-based kanban board system designed for coding agents, with a CLI, a terminal UI (TUI), a WebUI, and an MCP server. Boards are stored authoritatively in SQLite, with markdown card files maintained as synchronized exports.

## Features

- **Multi-project boards** — Initialize separate kanban boards per project
- **Markdown card exports** — Each card is exported as a human-readable markdown file with YAML frontmatter; direct markdown edits are not imported
- **SQLite persistence** — Fast, portable, zero-config database
- **Terminal UI (TUI)** — Full keyboard-driven TUI with 3-panel layout (columns / cards / detail)
- **WebUI** — Browser-based single-project kanban board with REST API, SSE updates between clients on the same server, keyboard navigation, drag-and-drop, modals, and search
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
│            markdown/ (export writer)                      │
│   Parser │ Writer                                        │
└───────────────────────────────────────────────────────────┘
```

## Installation

Requires Rust 1.75+.

```bash
git clone https://github.com/AZQN0/kanban-tool.git
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
mkdir -p my-project
cd my-project
kanban init

# 2. Create some cards
kanban create --title "Fix login bug" --priority high
kanban create --title "Write tests" --priority medium --label backend

# 3. List everything
kanban list

# 4. Move a card
CARD_ID=$(kanban create --title "Review API docs" | awk '/^Created card:/ {print $3}')
kanban move "$CARD_ID" in_progress

# 5. Open the TUI (terminal UI)
kanban board

# 6. Open the WebUI (browser-based, requires --features webui)
kanban web-ui --port 8080

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

Move a card to a different column. Updates SQLite and refreshes the synchronized markdown export.

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

Removes the card from SQLite and removes its synchronized markdown export.

### `kanban board`

Launch the terminal UI (TUI). Run from within an initialized project directory.

### `kanban web-ui [OPTIONS]`

Launch the WebUI in a browser. Requires build with `--features webui`.
The WebUI serves the initialized project in the current working directory. It does not switch projects in-browser; run a separate `kanban web-ui` process from another project directory to view that board. The WebUI binds to `127.0.0.1` by default. Non-loopback bind addresses are refused unless you pass `--allow-remote`, because the WebUI exposes unauthenticated mutating API routes. No authentication is provided.

```bash
kanban web-ui                                      # Default: http://127.0.0.1:9876
kanban web-ui --port 8080                          # Custom local port
kanban web-ui --bind 0.0.0.0 --allow-remote        # Explicit remote bind
kanban web-ui --bind 0.0.0.0 --port 3000 --allow-remote
```

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `--bind` | `127.0.0.1` | Bind address. Non-loopback addresses require `--allow-remote` |
| `--port` | `9876` | Port to listen on |
| `--allow-remote` | `false` | Permit binding the unauthenticated WebUI to a non-loopback address |

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
│ ↑↓ Nav | Enter Focus | m Move | D Delete | q Quit        │
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
| `D` | Delete selected card |
| `P` | Switch project (project picker) |
| `/` | Start search |
| `q` | Quit |

## WebUI

Start with `kanban web-ui` from an initialized project directory (requires `--features webui`). Opens a browser-based kanban board for that single project. Mutations made through that running WebUI server are sent to other connected WebUI clients via Server-Sent Events. CLI and MCP changes are not pushed into already-open WebUI clients. It is local-only by default; pass `--allow-remote` only when you intentionally want to expose the unauthenticated API beyond loopback.

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

- **Single-project view** — The running server exposes the board from its current project directory
- **SSE updates** — Changes made through one WebUI client are broadcast to other clients connected to the same server
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
| `e` | Open edit modal |
| `/` | Open search input |
| `Escape` | Close modal / Cancel |

### WebUI REST API

When running, the WebUI exposes a REST API:

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/boards` | List boards stored in the current project's database |
| `GET` | `/api/cards` | Get all cards grouped by column |
| `GET` | `/api/cards/:id` | Get a single card |
| `POST` | `/api/cards` | Create a new card |
| `PATCH` | `/api/cards/:id` | Update card fields |
| `DELETE` | `/api/cards/:id` | Delete a card |
| `POST` | `/api/cards/:id/move` | Move card to column |
| `GET` | `/api/cards/search?q=term` | Search cards |
| `GET` | `/api/events` | SSE feed for WebUI API mutations on this server |

## MCP Server

Start with `kanban server` (runs over stdio). Exposes 8 tools:

| Tool | Required Args | Description |
|------|---------------|-------------|
| `create_card` | `project`, `title` | Create a new card |
| `get_card` | `card_id` (+ optional `project`) | Get full card data |
| `update_card` | `card_id` (+ optional `project` and fields) | Update card fields |
| `delete_card` | `card_id` (+ optional `project`) | Delete a card |
| `list_cards` | optional filters, including `project` | List cards |
| `transition_card` | `card_id`, `column` (+ optional `project`) | Move card to column |
| `search_cards` | `query` (+ optional `project`) | Search title/description |
| `manage_board` | `action` (+ optional `project`) | Init board, add/remove columns |

For MCP tools where `project` is optional, omitting it uses the MCP server's current working directory. Pass `project` to target a different initialized board, especially for mutating tools such as `update_card`, `delete_card`, and `transition_card`. In the current MCP schema, `create_card` requires an explicit `project`.

### Example: Create via MCP

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
  "name":"create_card",
  "arguments":{"project":"/path/to/proj","title":"New feature","priority":"medium"}
}}
```

## Data Model

### Card File

SQLite is the authoritative store. Each card is also synchronized to `.kanban/cards/<id>.md` as an export for reading, inspection, and repair workflows. Direct edits to these markdown files are not imported back into SQLite; edit cards through the CLI, WebUI, or MCP API.

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

## Current Limitations

- SQLite is the source of truth. Markdown card files are synchronized exports only, and direct markdown edits are not imported.
- The WebUI is single-project per running server. Start one `kanban web-ui` process per project directory.
- WebUI SSE updates are broadcast between clients connected to the same server after WebUI API mutations. The WebUI does not currently watch SQLite for CLI or MCP changes.
- The WebUI has no authentication. It binds to loopback by default; non-loopback binds require explicit `--allow-remote`.
- The TUI supports navigation, move, delete, project switching, and search. Edit card fields through the CLI, WebUI, or MCP API.

## Testing

### Unit & Integration Tests

```bash
cargo test --all-features
cargo clippy --all-features
```

### E2E Tests (WebUI)

Requires Playwright and a Chromium browser:

```bash
# Install dependencies from the tracked lockfile
npm ci
npx playwright install chromium

# Build the release WebUI binary used by e2e/run.sh
cargo build --release --features webui

# Run all E2E tests
bash e2e/run.sh
# or
npm run e2e
```

E2E tests cover page load, columns, cards, detail view, move/delete modals, keyboard navigation, search, messages, and empty state.

### Dependency Audit Notes

The npm manifest and lockfile are tracked for Playwright-based E2E tests. Package metadata uses a public repository URL and the MIT license from `LICENSE`; avoid tokenized repository URLs in npm metadata. Use `npm audit` for Node development dependencies and a Rust dependency audit tool such as `cargo audit` when it is installed.

## Development

```bash
cargo build
cargo test
cargo clippy
```

## ⚠️ Disclaimer

> This project was **entirely vibecoded with [Qwen3.6-35B-A3B-GGUF](https://huggingface.co/Qwen/Qwen3.6-35B-A3B-GGUF)**. The code works, but use at your own risk.
