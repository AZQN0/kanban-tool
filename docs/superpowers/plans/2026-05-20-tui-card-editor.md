# TUI Card Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real TUI card editor that updates SQLite through the shared persistence layer and keeps markdown exports synchronized.

**Architecture:** The TUI editor will be an in-app modal/form, not a markdown-file editor. It will edit a selected card by building a `CardPatch` and calling `update_card_with_markdown`, then reload the board snapshot on success so SQLite remains authoritative and markdown remains a synchronized export.

**Tech Stack:** Rust, ratatui, crossterm, existing `Store`, `CardPatch`, and `update_card_with_markdown` persistence APIs.

---

## Scope

Build a first-pass TUI editor for these card fields:

- `title`
- `description`
- `priority`
- `labels`

Out of scope for this plan:

- column moves, because `m` already handles movement
- subtasks
- parent-card relationships
- external `$EDITOR` integration
- importing direct markdown edits back into SQLite

## File Structure

- Modify `src/tui/app.rs` to own editor state, validation, save/cancel behavior, and persistence integration.
- Modify `src/tui/events.rs` to route keys into editor mode and bind `e` in normal mode.
- Modify `src/tui/render.rs` to render the editor modal and advertise `e Edit` only after editing works.
- Modify `README.md` and `skills/kanban/SKILL.md` to document supported TUI editing accurately.
- Add or extend TUI tests in `src/tui/events.rs`, `src/tui/render.rs`, and `src/tui/app.rs`.
- Add persistence-backed tests where needed to prove TUI saves update both SQLite and markdown exports.

## Tasks

### Task 1: Editor State and App API

**Goal:** Add editor data structures and app-level methods without changing key handling or rendering yet.

**Files:**
- Modify: `src/tui/app.rs` — add editor mode, state types, field navigation, validation, save/cancel methods.

**Design:**
- Extend `Mode`:
  - `Editing`
- Add:
  - `pub enum EditorField { Title, Description, Priority, Labels }`
  - `pub struct EditorState { card_id, field, title, description, priority, labels_input, dirty }`
- Add methods:
  - `pub fn start_editing_selected_card(&mut self) -> Result<()>`
  - `pub fn cancel_editor(&mut self)`
  - `pub fn editor_next_field(&mut self, forward: bool)`
  - `pub fn editor_insert_char(&mut self, ch: char)`
  - `pub fn editor_backspace(&mut self)`
  - `pub fn editor_cycle_priority(&mut self, forward: bool)`
  - `pub fn save_editor(&mut self) -> Result<()>`
- `save_editor` builds a `CardPatch`, calls `update_card_with_markdown`, reloads board data, restores selection to the updated card when possible, closes the editor, and sets a success message.

**Key behaviors to test:**
- Starting edit mode copies the selected card into `EditorState`.
- Starting edit mode with no selected card returns a clear error.
- Canceling clears editor state and leaves card data unchanged.
- Empty title validation fails before persistence.

**Implementation notes:**
- Store labels as comma-separated `labels_input` in the editor state, then parse into `Vec<String>` on save.
- Trim title and labels; keep description content as typed.
- Do not update local `all_cards` or `columns` until persistence succeeds and data is reloaded.

### Task 2: Editor Key Handling

**Goal:** Wire normal-mode `e` and editing-mode keys to the app editor API.

**Files:**
- Modify: `src/tui/events.rs` — add editor event routing and tests.

**Design:**
- In normal mode:
  - `e` calls `app.start_editing_selected_card()`.
- In `Mode::Editing`:
  - `Esc` calls `app.cancel_editor()`.
  - `Ctrl+s` calls `app.save_editor()`.
  - `Tab` moves to next editor field.
  - `BackTab` moves to previous editor field.
  - `Up` and `Down` move between fields.
  - `Left` and `Right` cycle priority when the active field is priority.
  - printable chars update the active text field.
  - `Backspace` removes text from the active text field.
  - `Enter` saves on title, priority, and labels; inserts a newline in description.

**Key behaviors to test:**
- Pressing `e` enters `Mode::Editing`.
- Pressing `Esc` cancels edit mode.
- Typing updates the active field.
- Priority cycles only while the priority field is active.
- `Ctrl+s` saves via app-level save behavior.

**Implementation notes:**
- Keep key handling simple and deterministic; do not introduce terminal raw-input special cases outside existing crossterm handling.
- Treat labels as plain text input in v1.

### Task 3: Editor Rendering

**Goal:** Render a usable editor modal and update status/help text truthfully.

**Files:**
- Modify: `src/tui/render.rs` — add editor modal rendering and update bottom status text.

**Design:**
- Add `render_editor(frame, app, area)`.
- When `app.mode == Mode::Editing`, render the normal board behind a centered editor modal.
- The modal shows:
  - title input
  - description textarea preview
  - priority value
  - labels comma-separated input
  - save/cancel hints
- Highlight the active `EditorField`.
- Add `e Edit` back to the normal-mode bottom status bar only after editor behavior exists.

**Key behaviors to test:**
- Editor modal displays all editable fields.
- Active field is visibly distinguishable in the test buffer.
- Normal status bar advertises `e Edit`.
- Editor status/help mentions save and cancel.

**Implementation notes:**
- Keep layout stable in small terminals by truncating long text and using fixed modal constraints.
- Do not nest decorative cards; use a single modal frame.

### Task 4: Persistence-Backed Save Tests

**Goal:** Prove TUI editing writes through the same safe path as CLI/WebUI/MCP.

**Files:**
- Modify: `src/tui/app.rs` tests or create focused test helpers in the same module.
- Use existing project fixture patterns from `src/persistence.rs` and `tests/integration_test.rs`.

**Design:**
- Create a temp initialized project.
- Create a card through existing store or persistence helpers.
- Build `App::new(project_path)`.
- Start editor, mutate fields, save.
- Assert:
  - SQLite card row changed.
  - markdown export changed.
  - stale markdown paths are not introduced.
  - app exits `Mode::Editing`.

**Key behaviors to test:**
- Save updates title, description, priority, and labels.
- Save rejects empty title and leaves persisted data untouched.
- Persistence error keeps editor open and reports an error.
- Reload after save keeps the edited card selected when possible.

**Implementation notes:**
- Prefer temp directories and the existing `init_board`/`Store` helpers.
- Avoid fragile terminal rendering assertions for persistence behavior.

### Task 5: Documentation and Usage Alignment

**Goal:** Document TUI editing accurately now that it exists.

**Files:**
- Modify: `README.md`
- Modify: `skills/kanban/SKILL.md`

**Design:**
- README TUI key table includes `e` as edit selected card.
- Current limitations no longer say edit fields only through CLI/WebUI/MCP.
- Keep the SQLite-source-of-truth wording: TUI editing writes through SQLite and updates markdown exports; direct markdown edits are still not imported.
- Skill docs describe the TUI edit workflow and keep markdown export caveat.

**Key behaviors to test:**
- Existing regression that docs do not advertise direct markdown editing still passes or is updated to enforce the new wording.
- Docs do not imply project switching.

**Implementation notes:**
- Be explicit that `e` edits card fields, not markdown files.

### Task 6: Full Verification and Cleanup

**Goal:** Ensure the feature is production-ready and does not reopen audit issues.

**Files:**
- No new feature files unless a prior task reveals a clear need.

**Design:**
- Run:
  - `cargo test --all-features`
  - `cargo fmt --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo audit`
  - `cargo build --release --features webui`
  - `npm test -- --reporter=list`
- Review docs and status bars for truthfulness.
- Confirm direct markdown editing is still not advertised.

**Key behaviors to test:**
- TUI editor saves through SQLite and markdown export path.
- TUI editor does not use direct markdown file editing.
- Existing CLI/WebUI/MCP update flows still pass.

**Implementation notes:**
- If tests expose broad TUI coupling, extract a small `editor` helper module only if it materially reduces complexity.
- Keep this feature focused; defer external editor integration to a separate plan.

## Self-Review

- Spec coverage: The plan covers editor state, key handling, rendering, persistence, docs, and verification.
- Placeholder scan: No placeholder tasks or undefined handoffs remain.
- Type consistency: `Mode::Editing`, `EditorState`, `EditorField`, `CardPatch`, and `update_card_with_markdown` are used consistently across tasks.
