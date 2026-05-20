# Kanban Tool Audit Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the kanban tool from local prototype quality to a safer, testable local application by fixing the verified security issues, API correctness gaps, MCP breakage, documentation drift, and verification hygiene found in the 2026-05-20 audit.

**Architecture:** Keep the existing Rust binary structure and repair the shared data layer first so CLI, TUI, WebUI, and MCP behavior stays consistent. Treat SQLite as the authoritative state unless a later product decision promotes markdown edits into a first-class import/sync feature. Keep the WebUI local-first, with explicit guards before any remote bind is allowed.

**Tech Stack:** Rust 2021, clap, rusqlite, axum/tower-http, ratatui/crossterm, rust-mcp-sdk, vanilla JavaScript, Playwright, npm, cargo fmt/clippy/test/audit.

---

## Audit Summary

The project is a functional local kanban prototype with four surfaces:

- CLI commands in `src/cli/*`
- TUI in `src/tui/*`
- MCP server in `src/mcp/*`
- Optional WebUI in `src/webui/*` and `webui/static/*`

The core persistence model is a SQLite database at `.kanban/kanban.db` plus markdown card exports in `.kanban/cards/`. The current implementation mostly treats SQLite as authoritative, while docs and UI copy imply markdown edits are synchronized back into the database. That mismatch is a source of correctness and user-expectation bugs.

### Verified Security Findings

1. **Committed credential material in project metadata and git remote**

   Evidence:
   - `package.json` contains a GitHub token in `repository.url`.
   - `git remote -v` prints the same token in the origin fetch/push URLs.

   Impact:
   - Anyone with access to the repo metadata, shell history, terminal output, or generated packages may obtain a reusable credential.
   - If the token is still valid, it can be used outside this project.

   Required fix:
   - Revoke the token outside the repo.
   - Replace all persisted URLs with tokenless HTTPS or SSH URLs.
   - Search tracked files and recent history for token patterns before publishing.

2. **WebUI static file path traversal**

   Evidence:
   - `src/webui/server.rs` joins the raw `*file` path segment into `static_dir()` and reads it.
   - Reproduction returned `200` and repo file contents for:

   ```text
   GET /static/%2e%2e/%2e%2e/Cargo.toml
   ```

   Impact:
   - Any file readable by the process and reachable relative to the selected static directory can be exposed over HTTP.
   - This is critical if the WebUI is bound beyond loopback.

   Required fix:
   - Serve static files with a safe static-file service or canonicalize and enforce that resolved paths stay under the static root.
   - Add a regression test for encoded traversal.

3. **Unauthenticated mutating WebUI API plus remote-bind documentation**

   Evidence:
   - `src/webui/server.rs` mounts unauthenticated `POST`, `PATCH`, and `DELETE` routes.
   - `README.md` documents `--bind 0.0.0.0`.

   Impact:
   - A remote user on the same network can create, edit, move, and delete cards if the server binds to a public interface.
   - A browser-based local attack can also target localhost endpoints unless origin/CSRF controls are added.

   Required fix:
   - Keep localhost as the default.
   - Add an explicit unsafe-public opt-in or refuse non-loopback binds by default.
   - Document the local-only security model.

4. **Potential destructive actions report success incorrectly**

   Evidence:
   - `DELETE /api/cards/not-a-card` returned `{"success":true}` with status `200`.
   - `Store::delete_card` does not check SQLite affected row count.

   Impact:
   - Clients cannot trust delete results.
   - Automation and MCP callers may hide user mistakes or race conditions.

   Required fix:
   - Return a typed not-found error when updates/deletes affect zero rows.
   - Apply the same row-count check to moves and updates where appropriate.

### API Correctness Findings

1. **HTTP status mapping is broken**

   Evidence:
   - `ApiError::bad_request` and `ApiError::not_found` store plain strings.
   - `IntoResponse` checks string prefixes that are never set.
   - Missing cards and bad columns returned `500` instead of `404` or `400`.

   Required fix:
   - Replace string-prefix inference with an enum or status field.
   - Add API tests for `400`, `404`, and `500`.

2. **WebUI move response can be stale**

   Evidence:
   - `move_card` reads the updated card after DB transition, clones it, rewrites markdown, then returns the originally read `updated`.

   Required fix:
   - Return the exact persisted card state after the move.
   - Avoid mutating cloned data solely for markdown sync.

3. **Partial DB/markdown writes can leave inconsistent state**

   Evidence:
   - Create/update/move write SQLite and markdown in separate operations.
   - Delete ignores markdown file removal errors in the WebUI and TUI paths.

   Required fix:
   - Define SQLite as authoritative.
   - Use transactions for DB state changes.
   - Treat markdown write failures as visible sync errors with repair guidance, or move markdown export to a retryable side effect.

### MCP Findings

1. **`update_card` can fail for normal updates without labels**

   Evidence:
   - `src/mcp/schema.rs` builds an SQL `WHERE id = ?` clause but does not append `card_id` to the params vector in the no-labels branch.

   Required fix:
   - Route MCP updates through `Store::update_card` instead of duplicating SQL.
   - Add MCP handler tests or lower-level tool-call tests.

2. **MCP responses hand-build JSON**

   Evidence:
   - Several response bodies are built with `format!(r#"{{...}}"#)`.

   Impact:
   - Card titles/descriptions containing quotes, backslashes, or newlines can produce invalid JSON.

   Required fix:
   - Construct `serde_json::Value` or response structs and serialize them.

3. **MCP project arguments are inconsistent**

   Evidence:
   - `create_card`, `get_card`, `list_cards`, and `search_cards` can accept project paths.
   - `update_card`, `delete_card`, and `transition_card` resolve only the current directory.

   Required fix:
   - Add optional `project` to all card-mutating MCP tools or document current-directory-only behavior clearly.
   - Prefer optional `project` for consistency.

### TUI and Usage Findings

1. **README command drift**

   Evidence:
   - README documents `kanban webui`.
   - The actual clap subcommand is `kanban web-ui`.

   Required fix:
   - Update docs and tests to use `web-ui`, or add a clap alias for `webui`.

2. **TUI edit behavior is misleading**

   Evidence:
   - Pressing `e` opens the card markdown file in `$EDITOR`.
   - The app reloads from SQLite afterward, not from the edited markdown.

   Impact:
   - User edits may appear to be accepted in the file but not reflected in the app.

   Required fix:
   - Either remove direct markdown editing from the TUI or implement explicit markdown import/reconciliation after editor exit.

3. **WebUI SSE is effectively disabled**

   Evidence:
   - The JS creates an `EventSource`, attaches listeners, and immediately closes it.
   - The app then polls every three seconds.

   Required fix:
   - Keep SSE connected with reconnect/backoff, or remove the SSE claim and route until real live updates are implemented.

4. **WebUI project picker does not switch projects**

   Evidence:
   - Clicking a project only reloads the page.
   - API state is bound to the server's startup project path.

   Required fix:
   - Remove project picker from WebUI for now, or introduce an explicit active project route and server-side project switching model.

### Performance Findings

1. **N+1 card queries for board rendering**

   Evidence:
   - WebUI and TUI list cards once per column.

   Impact:
   - Fine for small boards, but wasteful as boards grow.

   Required fix:
   - Fetch all cards for a board once and group by `column_id`.

2. **Blocking SQLite/file operations in async handlers**

   Evidence:
   - WebUI handlers perform synchronous `rusqlite` and filesystem work on the current-thread Tokio runtime.

   Impact:
   - Requests can block each other under concurrent use.

   Required fix:
   - For local-only use, document the limitation and keep endpoints fast.
   - If multi-client support is a goal, use `spawn_blocking` or a small DB worker abstraction.

3. **Search and label filtering are linear scans through unindexed patterns**

   Evidence:
   - Search uses `LIKE '%query%'`.
   - Label filtering searches JSON text with `LIKE`.

   Required fix:
   - Keep as-is for local MVP scale, but add query limits and document expected scale.
   - Add FTS5 or normalized labels only if boards become large.

### Code Quality and Verification Findings

1. **Formatting is not clean**

   Evidence:
   - `cargo fmt --check` failed broadly.

   Required fix:
   - Run `cargo fmt` and commit the formatting.

2. **Clippy with warnings denied fails**

   Evidence:
   - `cargo clippy --all-targets --all-features -- -D warnings` failed with 15 diagnostics.

   Required fix:
   - Address clippy diagnostics or intentionally scope allows with comments only where justified.

3. **Integration tests are placeholders**

   Evidence:
   - `tests/integration_test.rs` contains comments and `assert!(true)`.

   Required fix:
   - Replace with real CLI/store integration tests.

4. **Application lockfile hygiene is wrong**

   Evidence:
   - `.gitignore` ignores `Cargo.lock`.
   - `package.json` and `package-lock.json` are untracked.
   - `node_modules/` is not ignored.

   Required fix:
   - Track `Cargo.lock` for this binary application.
   - Decide whether the Playwright harness is part of the repo; if yes, track npm manifests and ignore `node_modules/`.

5. **Dependency audit warnings exist**

   Evidence:
   - `cargo audit` found no vulnerabilities.
   - It reported informational warnings: `lru` unsound via `ratatui`, `paste` unmaintained via `ratatui`, and `rustls-pemfile` unmaintained via `rust-mcp-sdk`.
   - `npm audit` found zero vulnerabilities.

   Required fix:
   - Track upstream dependency upgrades.
   - Prefer upgrading `ratatui` and `rust-mcp-sdk` when compatible versions remove those advisories.

## Architecture Decisions

1. **SQLite remains authoritative**

   The current app already reads operational state from SQLite. Keep that model and make markdown a synchronized export until a separate markdown-import feature is designed. This avoids hidden data-loss bugs where hand-edited markdown silently conflicts with database state.

2. **WebUI remains local-first**

   The WebUI can be safe and useful as a loopback-only tool. Remote operation requires authentication, CSRF/origin controls, TLS/proxy guidance, and stronger operational documentation, which is outside this remediation plan.

3. **Shared store errors drive all interfaces**

   CLI, TUI, WebUI, and MCP should use the same `Store` operations and typed errors for create/update/delete/move. Interface layers should translate those errors into user messages, HTTP status codes, or MCP tool errors without duplicating SQL.

4. **Test the public behavior first**

   Security and correctness fixes should have regression tests that reproduce the original bug: path traversal, wrong status codes, missing-row deletes, MCP JSON escaping, and real CLI command behavior.

## Fix Plan

### Task 1: Secret Removal and Repository Hygiene

**Goal:** Remove credential material, stabilize dependency inputs, and make the repo safe to share.

**Files:**
- Modify: `package.json` — replace tokenized repository URL with `https://github.com/AZQN0/kanban-tool.git` or `git@github.com:AZQN0/kanban-tool.git`.
- Modify: `.git/config` outside tracked files — replace tokenized origin fetch/push URLs with tokenless URLs.
- Modify: `.gitignore` — stop ignoring `Cargo.lock`; add `node_modules/`.
- Add/Track: `Cargo.lock` — commit lockfile for reproducible binary builds.
- Add/Track: `package.json` and `package-lock.json` if Playwright E2E tests remain part of the repo.

**Design:**
- Treat token revocation as an external prerequisite and perform it before pushing or publishing.
- Keep the npm test harness either fully tracked or fully removed. Since E2E tests are already tracked, the pragmatic path is to track `package.json` and `package-lock.json`.
- Add a lightweight secret scan command to the verification checklist:

```bash
rg -n "gho_|ghp_|github_pat_|xox[baprs]-|sk-[A-Za-z0-9]|AKIA[0-9A-Z]{16}|BEGIN (RSA|OPENSSH|EC|PRIVATE) KEY|password\\s*=|token\\s*=|secret\\s*=" -g '!target/**' -g '!node_modules/**' -g '!.worktrees/**' .
```

**Key behaviors to test:**
- `git remote -v` prints no credential material.
- `rg` secret scan returns no real secrets.
- `cargo build --locked --all-features` succeeds after `Cargo.lock` is tracked.
- `npm ci` succeeds from the tracked `package-lock.json`.

**Implementation notes:**
- Do not rewrite git history unless this repository has already been published with the token and the user explicitly approves history cleanup.
- Revoking the token is mandatory because removing it from files does not invalidate prior exposure.

### Task 2: Safe WebUI Static Serving and Local-Bind Guard

**Goal:** Eliminate path traversal and prevent accidental remote exposure of unauthenticated mutating routes.

**Files:**
- Modify: `src/main.rs` — add a guard around `Commands::WebUI` bind behavior.
- Modify: `src/webui/server.rs` — replace raw static path joining with safe serving.
- Test: add WebUI/API tests under a new Rust integration test file or extend Playwright with HTTP-level assertions.
- Modify: `README.md` — document local-only behavior and the correct command name.

**Design:**
- Use `tower_http::services::ServeDir` if compatible with current `tower-http`, or implement a strict canonicalization helper:

```rust
fn resolve_static_path(static_root: &Path, requested: &str) -> Result<PathBuf, StaticFileError>
```

- The helper must:
  - reject absolute paths,
  - reject path components that are `ParentDir`,
  - URL-decoded traversal must not escape `static_root`,
  - canonicalize the candidate and require `candidate.starts_with(static_root_canonical)`.
- Add a bind guard:

```rust
fn validate_webui_bind(bind: &str, allow_remote: bool) -> Result<()>
```

- Add a clap flag if remote bind support is kept:

```rust
WebUI {
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,
    #[arg(long, default_value_t = 9876)]
    port: u16,
    #[arg(long)]
    allow_remote: bool,
}
```

**Key behaviors to test:**
- `GET /static/app.js` returns `200`.
- `GET /static/%2e%2e/%2e%2e/Cargo.toml` returns `404` or `403` and never includes file contents.
- `kanban web-ui --bind 0.0.0.0` fails with a clear message unless `--allow-remote` is provided.
- README examples use `kanban web-ui`, or `webui` works as an alias if that approach is chosen.

**Implementation notes:**
- Prefer refusing non-loopback binds over silently warning. A warning is easy to miss in automation.
- Do not add authentication in this task unless remote bind becomes a supported product goal.

### Task 3: Typed Store Errors and Correct HTTP Status Codes

**Goal:** Make missing cards, invalid input, and internal failures visible and correctly classified across all interfaces.

**Files:**
- Modify: `src/board/store.rs` — add typed store errors or consistently return `anyhow` contexts from typed helper functions.
- Modify: `src/webui/api.rs` — replace string-prefix `ApiError` status inference with explicit status.
- Modify: `src/cli/*` — preserve clear user-facing errors from store operations.
- Modify: `src/tui/app.rs` and `src/tui/events.rs` — display store errors without swallowing them.
- Test: add API/store tests for not-found and bad-input cases.

**Design:**
- Define an explicit API error shape:

```rust
pub enum ApiErrorKind {
    BadRequest,
    NotFound,
    Internal,
}

pub struct ApiError {
    pub kind: ApiErrorKind,
    pub error: String,
}
```

- Map kinds to statuses in `IntoResponse`.
- Update store write methods to check affected rows:

```rust
pub fn delete_card(&mut self, card_id: &str) -> Result<()>
pub fn transition_card(&mut self, card_id: &str, new_column_id: &str) -> Result<()>
pub fn update_card(...) -> Result<()>
```

- If a write affects zero rows, return a not-found style error that interface layers can translate.

**Key behaviors to test:**
- `GET /api/cards/not-a-card` returns `404`.
- `DELETE /api/cards/not-a-card` returns `404`.
- `POST /api/cards` with an unknown column returns `400`.
- `POST /api/cards/not-a-card/move` returns `404`.
- Successful delete returns `200` only when a row was actually deleted.

**Implementation notes:**
- Keep response JSON stable: `{ "error": "..." }` for errors and `{ "success": true }` for successful delete is acceptable.
- Avoid inspecting error message strings to determine status.

### Task 4: Atomic Persistence and Markdown Export Semantics

**Goal:** Prevent SQLite/markdown divergence from being hidden and make the markdown role explicit.

**Files:**
- Modify: `src/board/store.rs` — expose transaction-friendly operations or a higher-level persistence service.
- Modify: `src/markdown/writer.rs` — add safe write behavior using a temporary file and rename.
- Modify: `src/webui/api.rs`, `src/cli/create.rs`, `src/cli/update.rs`, `src/cli/delete.rs`, `src/cli/transition.rs`, `src/tui/app.rs`, `src/mcp/schema.rs` — route writes through shared persistence helpers.
- Modify: `README.md` and `skills/kanban/SKILL.md` — state that markdown files are synchronized exports, not the authoritative edit path.

**Design:**
- Add a shared service module if needed:

```rust
pub mod persistence;

pub fn create_card_with_markdown(store: &mut Store, card: &Card, cards_dir: &Path) -> Result<String>
pub fn update_card_with_markdown(store: &mut Store, card_id: &str, patch: CardPatch, cards_dir: &Path) -> Result<Card>
pub fn move_card_with_markdown(store: &mut Store, card_id: &str, column_id: &str, cards_dir: &Path) -> Result<Card>
pub fn delete_card_with_markdown(store: &mut Store, card_id: &str, cards_dir: &Path) -> Result<Card>
```

- Define a small patch type:

```rust
pub struct CardPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub column_id: Option<String>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
}
```

- For markdown writes:
  - write to `<id>.md.tmp`,
  - flush,
  - rename to `<id>.md`,
  - return an explicit error if export fails.

**Key behaviors to test:**
- Creating a card creates both DB row and markdown file.
- Updating a card updates both DB row and markdown file.
- Deleting a card removes the row and file, and errors are visible if file removal fails.
- A markdown export failure does not report full success.

**Implementation notes:**
- A fully atomic DB-plus-filesystem transaction is not possible with SQLite and ordinary file writes. The goal is honest reporting and easy repair, not impossible cross-resource atomicity.
- Keep markdown import out of scope for this remediation unless Task 7 chooses to support TUI markdown edits.

### Task 5: MCP Tool Correctness and JSON Serialization

**Goal:** Make MCP tools reliable, project-consistent, and safe for arbitrary card text.

**Files:**
- Modify: `src/mcp/schema.rs` — remove duplicated SQL and hand-built JSON.
- Test: add MCP tool-call unit tests or integration tests that call the tool structs directly.
- Modify: `README.md` and `skills/kanban/SKILL.md` — document the final project argument behavior.

**Design:**
- Add optional `project: Option<String>` to mutating card tools that currently lack it:

```rust
pub struct UpdateCardTool {
    pub card_id: String,
    pub project: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub column: Option<String>,
    pub priority: Option<String>,
    pub labels: Option<Vec<String>>,
}
```

- Use shared store/persistence operations from Task 4.
- Define response structs:

```rust
#[derive(Serialize)]
struct CardSummaryResponse {
    card_id: String,
    title: String,
    column: String,
}
```

- Serialize with `serde_json::to_string`.

**Key behaviors to test:**
- `update_card` without labels updates the title successfully.
- Card titles containing quotes and newlines produce valid JSON responses.
- `delete_card` with a `project` argument deletes from the requested project.
- Unknown column returns an MCP tool error with available columns.

**Implementation notes:**
- Keep MCP error messages readable for agents.
- Avoid broad `unwrap_or_default()` on serialization paths where data loss would hide a bug.

### Task 6: Query Efficiency and Shared Board Loading

**Goal:** Remove repeated card queries and establish one board-loading path for CLI/TUI/WebUI.

**Files:**
- Modify: `src/board/store.rs` — add a board snapshot or grouped card query.
- Modify: `src/webui/api.rs` — use grouped board snapshot for `/api/cards`.
- Modify: `src/tui/app.rs` — use grouped board snapshot for initial load and reload.
- Test: add store tests for grouped cards preserving column order and card sorting.

**Design:**
- Add a shared response type:

```rust
pub struct BoardSnapshot {
    pub board: Board,
    pub columns: Vec<ColumnCards>,
    pub all_cards: Vec<Card>,
}

pub struct ColumnCards {
    pub column: Column,
    pub cards: Vec<Card>,
}
```

- Store method:

```rust
pub fn load_board_snapshot(&self, project_path: &str, sort_by: CardSort) -> Result<BoardSnapshot>
```

- Internally:
  - load board and columns once,
  - load all cards for the board once,
  - group cards by `column_id`,
  - emit columns in configured sort order.

**Key behaviors to test:**
- Empty columns are present.
- Cards appear under the correct column.
- Priority sorting matches current behavior.
- WebUI `/api/cards` output remains compatible with existing JS.

**Implementation notes:**
- Preserve JSON field names expected by `webui/static/app.js`.
- Keep search as a separate query for now.

### Task 7: Fix or Remove Misleading TUI Markdown Editing

**Goal:** Ensure pressing `e` either edits real authoritative data or is removed from the advertised workflow.

**Files:**
- Modify: `src/tui/app.rs` — change edit behavior.
- Modify: `src/tui/events.rs` — reload and error behavior after edit.
- Modify: `src/tui/render.rs` — update keybinding text if edit is removed or changed.
- Modify: `README.md` — align TUI keybinding docs.
- Test: add focused unit tests for parser/import if markdown edit is retained.

**Design Option A: Remove direct editor integration**
- Remove `e` from keybindings and UI copy.
- Keep card edits in CLI/WebUI only.
- This is the lowest-risk path because markdown import is not productized.

**Design Option B: Support markdown import after editor exit**
- Spawn editor and wait for exit.
- Parse the edited markdown file.
- Validate card id, board id, column id, priority, labels.
- Apply parsed changes to SQLite via shared update path.
- Report parse/validation errors in the TUI.

**Recommended choice:** Option A for this remediation. Option B should be a separate feature because it creates conflict-resolution and validation requirements.

**Key behaviors to test:**
- If Option A: TUI no longer advertises or responds to `e`.
- If Option B: editing title/description in markdown updates SQLite and re-renders the card.
- Invalid markdown does not corrupt SQLite.

**Implementation notes:**
- Do not leave a keybinding that opens files but cannot affect application state.

### Task 8: WebUI Live Updates and Project Picker Truthfulness

**Goal:** Align WebUI behavior with what it claims to support.

**Files:**
- Modify: `webui/static/app.js` — fix SSE or remove event-source setup.
- Modify: `src/webui/sse.rs` — keep only if SSE remains supported.
- Modify: `webui/static/index.html` and `webui/static/style.css` — remove or simplify project picker if needed.
- Modify: `README.md` — document actual WebUI capabilities.
- Test: extend Playwright tests for SSE behavior or remove SSE claims from tests/docs.

**Design:**
- For SSE:
  - keep one active `EventSource`,
  - on events, call `refreshCurrentColumn` or `refreshAll`,
  - on error, close and reconnect with backoff,
  - do not also poll every three seconds unless as a documented fallback.
- For project picker:
  - remove picker if the server remains single-project,
  - or introduce route/query state if multi-project WebUI becomes a supported feature.

**Recommended choice:** Keep the server single-project and remove the project picker from WebUI for this remediation. Multi-project switching should be a separate design because each project currently has its own `.kanban/kanban.db`.

**Key behaviors to test:**
- Opening two WebUI pages and moving a card in one updates the other if SSE remains.
- If project picker is removed, no UI advertises switching projects.
- README no longer promises unsupported project switching.

**Implementation notes:**
- Do not keep dead SSE code that immediately closes the connection.

### Task 9: Documentation and Skill Alignment

**Goal:** Make README and agent skill instructions match the real binary, security model, and supported workflows.

**Files:**
- Modify: `README.md` — commands, WebUI security, testing, current limitations, dependency audit notes.
- Modify: `skills/kanban/SKILL.md` — command spelling, markdown semantics, MCP tool argument behavior.
- Modify: `package.json` — license and repository metadata if npm manifests stay tracked.

**Design:**
- Update command references from `webui` to `web-ui`, unless a clap alias is added.
- Replace "live updates via SSE" with either accurate SSE behavior or "periodic refresh" depending on Task 8.
- Clarify markdown:
  - database is authoritative,
  - markdown files are synchronized exports,
  - direct markdown edits are not imported unless a future feature adds it.
- Clarify security:
  - WebUI is local-only by default,
  - remote bind requires explicit opt-in,
  - no authentication is provided.

**Key behaviors to test:**
- Every README command can be copied and run.
- Skill examples use valid CLI commands.
- Testing section matches actual verification commands and outcomes.

**Implementation notes:**
- Remove claims like "clean build, no warnings" until CI enforces them.

### Task 10: Verification Coverage and CI-Ready Checks

**Goal:** Replace placeholder tests and make the project reliably verifiable before commits.

**Files:**
- Replace: `tests/integration_test.rs` — real integration tests for init/create/list/move/delete/search.
- Add: `tests/webui_api_test.rs` behind `webui` feature if practical.
- Modify: `e2e/basic.spec.js` and `e2e/helper.js` — cover command spelling and critical WebUI flows.
- Add: optional CI workflow if the repo uses GitHub Actions.

**Design:**
- Integration tests should use temporary project directories and the compiled binary where useful.
- Store/API tests should run without requiring a browser.
- E2E remains browser-focused and covers user-visible behavior.
- Verification command set:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --features webui
npm ci
npm audit --audit-level=moderate
npm test -- --reporter=list
cargo audit
```

**Key behaviors to test:**
- CLI lifecycle: init, create, list, get, move, update, search, delete.
- WebUI security: traversal blocked, bad input returns `400`, missing resources return `404`.
- MCP JSON serialization with special characters.
- Regression for the no-labels MCP update path.

**Implementation notes:**
- Keep tests deterministic and use temp directories.
- If `cargo audit` is not available in CI, document installation or use an action that provides it.

### Task 11: Dependency Cleanup and Upgrade Pass

**Goal:** Reduce advisory noise and unused dependency surface without disrupting core behavior.

**Files:**
- Modify: `Cargo.toml` — remove unused dependencies and upgrade where safe.
- Modify: `Cargo.lock` — update after dependency changes.
- Modify: `package.json` and `package-lock.json` — keep Playwright versions pinned by lockfile.

**Design:**
- Check which dependencies are unused:
  - `serde_yaml` appears unused in production parser code.
  - `console` appears unused.
  - `fallible-streaming-iterator` may be unnecessary as a direct dependency.
  - `futures`, `tokio-stream`, and `async-trait` should be verified against current MCP/WebUI code.
- Try upgrading:
  - `ratatui` to a version that avoids the `lru` and `paste` advisories if available and compatible,
  - `rust-mcp-sdk` to a version that avoids `rustls-pemfile` if available and compatible,
  - `tower-http` to current compatible version if using `ServeDir`.

**Key behaviors to test:**
- Full verification command set passes after dependency updates.
- TUI still renders and handles keyboard input.
- MCP server still starts and lists tools.
- WebUI still serves static files and API routes.

**Implementation notes:**
- Do this after functional fixes so dependency changes do not obscure behavior changes.
- Keep upgrades incremental and revert only the dependency change if a package upgrade causes broad breakage.

## Suggested Execution Order

1. Task 1: Secret Removal and Repository Hygiene
2. Task 2: Safe WebUI Static Serving and Local-Bind Guard
3. Task 3: Typed Store Errors and Correct HTTP Status Codes
4. Task 4: Atomic Persistence and Markdown Export Semantics
5. Task 5: MCP Tool Correctness and JSON Serialization
6. Task 6: Query Efficiency and Shared Board Loading
7. Task 7: Fix or Remove Misleading TUI Markdown Editing
8. Task 8: WebUI Live Updates and Project Picker Truthfulness
9. Task 9: Documentation and Skill Alignment
10. Task 10: Verification Coverage and CI-Ready Checks
11. Task 11: Dependency Cleanup and Upgrade Pass

Tasks 1 and 2 are security blockers and should be done before any publishing or remote use. Tasks 3 through 5 repair shared correctness risks. Tasks 6 through 11 improve maintainability, usage truthfulness, and long-term confidence.

## Acceptance Criteria

- No credential material remains in tracked files, git remotes, or project metadata.
- Path traversal under `/static/*file` is blocked by a regression test.
- Non-loopback WebUI binds are refused unless explicitly opted into.
- WebUI API returns `400` for invalid input, `404` for missing cards, and `500` only for internal failures.
- Missing-row update/delete/move operations do not report success.
- MCP tools serialize JSON with `serde_json` and support consistent project targeting.
- README and `skills/kanban/SKILL.md` match actual command names and supported behavior.
- `cargo fmt --check` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- `cargo test --all-features` passes with real integration coverage.
- `cargo build --release --features webui` passes.
- `npm ci`, `npm audit --audit-level=moderate`, and `npm test -- --reporter=list` pass.
- `cargo audit` reports no vulnerabilities; informational warnings are documented or resolved.

## Current Verification Snapshot

Commands run during audit:

```bash
cargo test --all-features
cargo build --release --features webui
npm ci
npm test -- --reporter=list
npm audit --audit-level=moderate
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo audit
```

Observed results:

- `cargo test --all-features`: passed, but integration coverage is a placeholder.
- `cargo build --release --features webui`: passed.
- `npm ci`: passed.
- `npm test -- --reporter=list`: 12 Playwright tests passed.
- `npm audit --audit-level=moderate`: 0 vulnerabilities.
- `cargo fmt --check`: failed.
- `cargo clippy --all-targets --all-features -- -D warnings`: failed with 15 diagnostics.
- `cargo audit`: 0 vulnerabilities, with informational warnings for `lru`, `paste`, and `rustls-pemfile`.

## Self-Review

Spec coverage:
- Security findings are covered by Tasks 1, 2, and 3.
- Performance findings are covered by Tasks 6 and 11.
- Code quality findings are covered by Tasks 9, 10, and 11.
- Usage findings are covered by Tasks 7, 8, and 9.

Placeholder scan:
- The plan avoids deferred placeholders and defines concrete files, interfaces, behaviors, and verification commands.

Type consistency:
- Shared types introduced in the plan are `ApiErrorKind`, `ApiError`, `CardPatch`, `BoardSnapshot`, and `ColumnCards`.
- Store and persistence functions consistently return `Result`.
