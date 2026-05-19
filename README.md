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

## AI Agent Skill

This project includes an agent skill (`skills/kanban/SKILL.md`) that lets AI coding agents interact with kanban boards. The skill covers:

- All CLI commands (`init`, `create`, `list`, `move`, `search`)
- MCP server tool usage
- Common workflows (setup, create, track, move)
- Card file format reference

Point your agent's skill directory at `skills/` in this repo.

## Usage — CLI

### Initialize a Board

```bash
kanban init [PATH]
```

Creates a `.kanban/` directory with a SQLite database, `cards/` folder, and `columns/` folder. Default columns: `backlog`, `todo`, `in_progress`, `review`, `done`.

### Create a Card

```bash
kanban create --title "Fix login bug" \
  --description "User reports login timeout after 5 minutes" \
  --priority high \
  --column todo \
  --label backend \
  --label bug
```

### List Cards

```bash
kanban list                          # All cards
kanban list --priority high          # Filter by priority
kanban list --column in_progress     # Filter by column
kanban list --label security         # Filter by label
kanban list --project /path/to/proj  # Another project's board
```

### Move a Card

```bash
kanban move <CARD_ID> in_progress
```

Moves the card in both the database and the markdown file.

### Search Cards

```bash
kanban search "login"
kanban search "auth" --project /path/to/proj
```

Searches across card titles and descriptions.

### Terminal UI

```bash
kanban board
```

### MCP Server

```bash
kanban server
```

Starts an MCP server over stdio with 8 tools. See [MCP Tools](#mcp-tools) below.

## Usage — Terminal UI

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
| `j` / `↓` | Move selection down (cards), down column (column list) |
| `k` / `↑` | Move selection up (cards), up column (column list) |
| `h` / `←` | Focus previous panel |
| `l` / `→` | Focus next panel |
| `Enter` | Focus selected card in detail pane / Unfocus detail |
| `Esc` | Cancel current mode / Unfocus detail |
| `m` | Move selected card (popup: `b`acklog, `t`odo, `i`n_progress, `r`eview, `d`one) |
| `e` | Open card in `$EDITOR` |
| `D` | Delete selected card |
| `P` | Switch project (shows project picker) |
| `/` | Start search (type query, Enter to search) |
| `q` | Quit |

## Usage — MCP Server

The MCP server exposes 8 tools over stdio.

### Tools

| Tool | Description | Required Args |
|------|-------------|---------------|
| `create_card` | Create a new card | `project`, `title` |
| `get_card` | Get a card by ID | `card_id` |
| `update_card` | Update card fields | `card_id` |
| `delete_card` | Delete a card by ID | `card_id` |
| `list_cards` | List cards with filters | — |
| `transition_card` | Move card to column | `card_id`, `column` |
| `search_cards` | Search across title/description | `query` |
| `manage_board` | Init board, add/remove columns | `action` |

### Example: Create a Card via MCP

```
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
  "name":"create_card",
  "arguments":{"project":"/path/to/proj","title":"New feature","priority":"medium"}
}}
```

### Example: List Cards via MCP

```
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
  "name":"list_cards",
  "arguments":{"project":"/path/to/proj","column":"todo","priority":"high"}
}}
```

## Data Model

### Card (Markdown File)

Each card is stored as `.kanban/cards/<id>.md`:

```markdown
---
id: uuid-v4
board_id: uuid-v4
column_id: uuid-v4
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

### SQLite Database

Stored at `.kanban/kanban.db`. Tables: `boards`, `columns`, `cards`, `comments`.

### Project Structure

```
my-project/
├── .kanban/
│   ├── kanban.db          # SQLite database
│   ├── cards/             # Markdown card files
│   │   ├── <uuid>.md
│   │   └── ...
│   └── columns/           # Column definitions (JSON)
│       └── <uuid>.json
└── ...
```

## Development

### Build

```bash
cargo build
cargo build --release
```

### Run Tests

```bash
cargo test
```

### Run Linter

```bash
cargo clippy
```

## ⚠️ Disclaimer

> This project was **entirely vibecoded with [Qwen3.6-35B-A3B-GGUF](https://huggingface.co/Qwen/Qwen3.6-35B-A3B-GGUF)** — an LLM. The code works, but treat it like a chatbot wrote it: review before using in production, don't expect architecture textbooks, and enjoy the vibes.

## ⚠️ Disclaimer

> This project was **entirely vibecoded with [Qwen3.6-35B-A3B-GGUF](https://huggingface.co/Qwen/Qwen3.6-35B-A3B-GGUF)**. The code works, but use at your own risk.

## ⚠️ Disclaimer

> This project was **entirely vibecoded with [Qwen3.6-35B-A3B-GGUF](https://huggingface.co/Qwen/Qwen3.6-35B-A3B-GGUF)**. The code works, but use at your own risk.

## Known Limitations

- `kanban list --label` does not filter by label yet (SQL parameter not wired up)
- No `delete`/`get`/`update` CLI subcommands — use `kanban server` (MCP) or direct markdown file edits
- SQLite WAL lock may fail if multiple `kanban` processes start simultaneously (transient)
