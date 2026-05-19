---
name: kanban
description: >
  Use for task tracking, project board management, and organizing work items.
  Auto-triggers on keywords like "kanban", "board", "task tracking", "card management",
  "move to todo", "backlog", "sprint", "work items", "track tasks", "create card",
  "what's in progress", "tasks", "issue tracking", "project board".
  Uses the kanban CLI or MCP server to manage multi-project kanban boards.
---

# Kanban Board Skill

Manage kanban boards for tracking tasks, organizing work, and monitoring project progress.

## Binary Location

After building the project (`cargo build`), the binary is at:

```
./target/debug/kanban
```

For release builds: `./target/release/kanban`

If installed system-wide (e.g. via `cargo install`), use `kanban` directly.

## Board Structure

Each project has its own board stored in `.kanban/`:

```
my-project/
├── .kanban/kanban.db          # SQLite database
├── .kanban/cards/             # Markdown card files (one per card)
├── .kanban/columns/           # Column definitions
│
└── default columns:
    backlog → todo → in_progress → review → done
```

Cards are stored as both markdown files (`.kanban/cards/<uuid>.md`) and in SQLite.

## CLI Commands

### Initialize a Board

```bash
kanban init [PATH]
# PATH defaults to current directory
```

Creates a `.kanban/` directory with default columns.

### Create a Card

```bash
kanban create --title "Title" \
  --description "Description" \
  --priority high \
  --column todo \
  --label backend \
  --project /path/to/project
```

**Required:** `--title`
**Optional:** `--project`, `--description`, `--column` (default: `todo`), `--priority` (default: `backlog`), `--label` (repeat for multiple)

**Priority values:** `backlog`, `low`, `medium`, `high`, `urgent`
**Column values:** `backlog`, `todo`, `in_progress`, `review`, `done`

### List Cards

```bash
kanban list
kanban list --priority high
kanban list --column in_progress
kanban list --label security
kanban list --project /path/to/project
```

### Move a Card

```bash
kanban move <CARD_ID> <COLUMN>
```

Updates both the database and the markdown file.

### Get a Card

```bash
kanban get <CARD_ID>
```

Prints full card details (title, column, priority, labels, description, timestamps).

### Update a Card

```bash
kanban update <CARD_ID> \
  --title "New title" \
  --description "New description" \
  --column in_progress \
  --priority urgent \
  --label backend
```

Update any combination of fields. All options are optional — only specify the fields you want to change.

### Delete a Card

```bash
kanban delete <CARD_ID>
```

Removes the card from both SQLite and the markdown file system.

### Search Cards

```bash
kanban search "keyword"
kanban search "auth" --project /path/to/project
```

Searches across card titles and descriptions.

## MCP Server Tools

Start with `kanban server` (runs over stdio). Available tools:

| Tool | Args | Description |
|------|------|-------------|
| `create_card` | `project`, `title` (+ optional `description`, `priority`, `column`, `labels`) | Create a new card |
| `get_card` | `card_id` (+ optional `project`) | Get full card data |
| `update_card` | `card_id` (+ optional fields) | Update card fields |
| `delete_card` | `card_id` | Delete a card |
| `list_cards` | optional filters | List cards |
| `transition_card` | `card_id`, `column` | Move card to column |
| `search_cards` | `query` (+ optional `project`) | Search cards |
| `manage_board` | `action` (+ optional fields) | Init board, add/remove columns |

## Common Workflows

### 1. Set Up a Project Board

```bash
cd /path/to/project
kanban init
```

### 2. Create Work Items

For each task or feature, create a card:

```bash
kanban create --title "Implement login endpoint" \
  --description "Add POST /api/auth/login with JWT token generation" \
  --priority high \
  --label backend \
  --label security
```

### 3. Track Progress

```bash
# Check what's in progress
kanban list --column in_progress

# Check high-priority items
kanban list --priority high

# Search for specific items
kanban search "auth"
```

### 4. Move Cards Through Workflow

```bash
kanban move <CARD_ID> in_progress   # Start working on it
kanban move <CARD_ID> review         # Ready for review
kanban move <CARD_ID> done           # Completed
```

### 5. Inspect and Update Cards

```bash
# Get full details of a card
kanban get <CARD_ID>

# Update a card's fields
kanban update <CARD_ID> --priority urgent --description "Critical issue"

# Remove a card no longer needed
kanban delete <CARD_ID>
```

### 6. Manage with MCP

Use the MCP server tools programmatically. Example:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
  "name":"create_card",
  "arguments":{"project":"/path/to/proj","title":"New feature","priority":"medium"}
}}
```

## Tips

- **Card IDs** are UUIDs returned by `kanban create`. Keep them handy for `move`, `get`, `update`, `delete`, and other operations.
- **Markdown cards** are human-editable — you can edit `.kanban/cards/<id>.md` directly and the DB stays in sync on next read.
- **Multi-project** — use `--project` flag or `kanban board` (TUI) to switch between boards.
- **Labels** — add multiple with repeated `--label` flags for categorization. Multiple labels filter as AND (card must have all specified labels).
- **Priority ordering** — `urgent` > `high` > `medium` > `low` > `backlog`.

## Card File Format

Each card is a markdown file with YAML frontmatter:

```markdown
---
id: <uuid>
board_id: <uuid>
column_id: <uuid>
title: Card title
priority: medium
labels: [backend, bug]
subtasks: []
parent_card_id: null
created_at: 2026-05-19T...
updated_at: 2026-05-19T...
---

Card description body (markdown)
```
