# Kanban — A Kanban Board for Coding Agents

A Rust-based kanban board system designed for coding agents, with both a CLI and a terminal UI (TUI). Boards are stored as SQLite databases with markdown card files, and an MCP server is included for integration with AI coding tools.

## Features

- **Multi-project boards** — Initialize separate kanban boards per project
- **Markdown-backed cards** — Each card is a human-readable markdown file with YAML frontmatter
- **SQLite persistence** — Fast, portable, zero-config database
- **Terminal UI** — Full keyboard-driven TUI with 3-panel layout (columns / cards / detail)
- **MCP Server** — 8 tools for programmatic card management (create, get, update, delete, list, move, search, manage)
- **CLI** — All operations available from the command line
- **Filters** — List cards by column, priority, or labels

## Architecture

```
┌─────────────────────────────────────────────────┐
│                    kanban                        │
├────────────┬────────────┬───────────────────────┤
│    CLI     │    TUI     │    MCP Server         │
│ (clap)     │(ratatui)   │ (rust-mcp-sdk)        │
├────────────┴────────────┴───────────────────────┤
│                board/ (data layer)              │
│   Store (SQLite) │ Card │ Column │ Label         │
├─────────────────────────────────────────────────┤
│              kanban/ (project manager)          │
│   Manager │ Config │ Init                       │
├─────────────────────────────────────────────────┤
│            markdown/ (persistence)              │
│   Parser │ Writer                               │
└─────────────────────────────────────────────────┘
```

## Installation

Requires Rust 1.75+.

```bash
git clone <repo>
cd kanban-tool/.worktrees/kanban-tool
cargo build --release
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

## Development

```bash
cargo build
cargo test
cargo clippy
```

## ⚠️ Disclaimer

> This project was **entirely vibecoded with [Qwen3.6-35B-A3B-GGUF](https://huggingface.co/Qwen/Qwen3.6-35B-A3B-GGUF)**. The code works, but use at your own risk.

## Known Limitations

- `kanban list --label` does not filter by label yet (SQL parameter not wired up)
- No `delete`/`get`/`update` CLI subcommands — use `kanban server` (MCP) or direct markdown edits
- SQLite WAL lock may fail if multiple `kanban` processes start simultaneously (transient)
