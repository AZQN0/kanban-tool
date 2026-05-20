#![allow(dead_code)]

use anyhow::{bail, Context, Result};
use crossterm::event::{self, Event as CEvent, KeyEventKind};
use std::path::PathBuf;

use crate::board::card::{Card, Priority};
use crate::board::store::{CardSort, Store};
use crate::kanban::config::{cards_dir, db_path, is_initialized};
use crate::persistence::{
    delete_card_with_markdown, move_card_with_markdown, update_card_with_markdown, CardPatch,
};

use super::events;
use super::render;

/// Which panel currently has focus.
#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Columns,
    Cards,
    Detail,
}

/// Mode the TUI is in.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Moving,
    Searching,
    SearchingResult,
    Editing,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditorField {
    Title,
    Description,
    Priority,
    Labels,
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pub card_id: String,
    pub field: EditorField,
    pub editing_text: bool,
    pub text_edit_original: Option<String>,
    pub title: String,
    pub title_cursor: usize,
    pub description: String,
    pub description_cursor: usize,
    pub priority: Priority,
    pub labels_input: String,
    pub labels_cursor: usize,
    pub dirty: bool,
}

/// The main application state.
pub struct App {
    pub running: bool,
    pub focus: Focus,
    pub mode: Mode,
    pub error: Option<String>,

    // Board data
    pub project_path: PathBuf,
    pub board_name: String,
    pub columns: Vec<ColumnView>,
    pub all_cards: Vec<Card>,

    // Current column index being viewed
    pub current_column_idx: usize,

    // Card selection within the current column
    pub card_selection: usize,

    // Detail view
    pub detail_card: Option<Card>,

    // Search state
    pub search_query: String,
    pub search_results: Vec<Card>,

    // Editor state
    pub editor: Option<EditorState>,

    // Message display (temporary)
    pub message: Option<String>,
    pub message_time: std::time::Instant,
}

/// A column with its cards pre-filtered.
#[derive(Debug)]
pub struct ColumnView {
    pub name: String,
    pub cards: Vec<Card>,
}

impl App {
    pub fn new(project_path: PathBuf) -> Result<Self> {
        if !is_initialized(&project_path) {
            bail!(
                "No kanban board found at {}. Run `kanban init {}` first.",
                project_path.display(),
                project_path.display()
            );
        }

        let db = db_path(&project_path);
        let store = Store::open(&db).context("Failed to open kanban database")?;
        let snapshot = store
            .load_board_snapshot(project_path.to_string_lossy().as_ref(), CardSort::Priority)
            .context("Failed to load board")?;

        let columns: Vec<ColumnView> = snapshot
            .columns
            .into_iter()
            .map(|column_cards| ColumnView {
                name: column_cards.column.name,
                cards: column_cards.cards,
            })
            .collect();

        let mut app = App {
            running: true,
            focus: Focus::Cards,
            mode: Mode::Normal,
            error: None,
            project_path,
            board_name: snapshot.board.name,
            columns,
            all_cards: snapshot.all_cards,
            current_column_idx: 0,
            card_selection: 0,
            detail_card: None,
            search_query: String::new(),
            search_results: Vec::new(),
            editor: None,
            message: None,
            message_time: std::time::Instant::now(),
        };
        app.normalize_focus_for_current_cards();
        Ok(app)
    }

    /// Reload the current column's cards from the store.
    pub fn reload_current_column(&mut self) -> Result<()> {
        let selected_card_id = self.selected_card_id();
        let detail_was_open = self.detail_card.is_some();
        let db = db_path(&self.project_path);
        let store = Store::open(&db)?;
        let snapshot = store.load_board_snapshot(
            self.project_path.to_string_lossy().as_ref(),
            CardSort::Priority,
        )?;
        let board_id = snapshot.board.id.clone();
        self.board_name = snapshot.board.name;
        self.columns = snapshot
            .columns
            .into_iter()
            .map(|column_cards| ColumnView {
                name: column_cards.column.name,
                cards: column_cards.cards,
            })
            .collect();
        self.all_cards = snapshot.all_cards;
        if self.mode == Mode::SearchingResult && !self.search_query.is_empty() {
            self.search_results = store.search_cards(&board_id, &self.search_query)?;
        }
        if !self.columns.is_empty() {
            self.current_column_idx = self.current_column_idx.min(self.columns.len() - 1);
        }
        if let Some(card_id) = selected_card_id {
            self.select_card_by_id(&card_id);
            if !detail_was_open {
                self.detail_card = None;
            }
        } else {
            self.normalize_focus_for_current_cards();
        }
        Ok(())
    }

    /// Reload all columns.
    pub fn reload_all(&mut self) -> Result<()> {
        let selected_card_id = self.selected_card_id();
        let detail_was_open = self.detail_card.is_some();
        let db = db_path(&self.project_path);
        let store = Store::open(&db)?;
        let snapshot = store.load_board_snapshot(
            self.project_path.to_string_lossy().as_ref(),
            CardSort::Priority,
        )?;
        let board_id = snapshot.board.id.clone();

        self.board_name = snapshot.board.name;
        self.columns = snapshot
            .columns
            .into_iter()
            .map(|column_cards| ColumnView {
                name: column_cards.column.name,
                cards: column_cards.cards,
            })
            .collect();
        self.all_cards = snapshot.all_cards;
        if self.mode == Mode::SearchingResult && !self.search_query.is_empty() {
            self.search_results = store.search_cards(&board_id, &self.search_query)?;
        }
        if !self.columns.is_empty() {
            self.current_column_idx = self.current_column_idx.min(self.columns.len() - 1);
        }
        if let Some(card_id) = selected_card_id {
            self.select_card_by_id(&card_id);
            if !detail_was_open {
                self.detail_card = None;
            }
        } else {
            self.normalize_focus_for_current_cards();
        }
        Ok(())
    }

    /// Get the cards for the current column (or search results if searching).
    pub fn current_cards(&self) -> &[Card] {
        match self.mode {
            Mode::SearchingResult => &self.search_results,
            _ => self
                .columns
                .get(self.current_column_idx)
                .map(|column| column.cards.as_slice())
                .unwrap_or(&[]),
        }
    }

    /// Move focus to the next/previous panel.
    pub fn focus_next(&mut self, forward: bool) {
        self.mode = Mode::Normal;
        self.detail_card = None;
        self.focus = match (forward, self.focus.clone()) {
            (true, Focus::Columns) => Focus::Cards,
            (true, Focus::Cards) => Focus::Detail,
            (true, Focus::Detail) => Focus::Columns,
            (false, Focus::Columns) => Focus::Detail,
            (false, Focus::Cards) => Focus::Columns,
            (false, Focus::Detail) => Focus::Cards,
        };
        if self.focus == Focus::Detail {
            self.detail_card = self.current_cards().get(self.card_selection).cloned();
        }
        self.normalize_focus_for_current_cards();
    }

    /// Move to the next/previous column.
    pub fn column_next(&mut self, forward: bool) {
        let cols = self.columns.len();
        if cols == 0 {
            return;
        }
        if forward {
            self.current_column_idx = (self.current_column_idx + 1) % cols;
        } else {
            self.current_column_idx = if self.current_column_idx == 0 {
                cols - 1
            } else {
                self.current_column_idx - 1
            };
        }
        self.card_selection = 0;
        self.detail_card = None;
        self.normalize_focus_for_current_cards();
    }

    /// Navigate within the current cards list.
    pub fn card_nav(&mut self, forward: bool) {
        let cards = self.current_cards();
        if cards.is_empty() {
            self.focus = Focus::Columns;
            self.detail_card = None;
            return;
        }
        let card_count = cards.len();
        let current = self.card_selection;
        let new_idx = if forward {
            (current + 1).min(card_count - 1)
        } else {
            current.saturating_sub(1)
        };
        // Get the card clone before the borrow ends
        let card_clone = cards.get(new_idx).cloned();
        self.card_selection = new_idx;
        // Focus detail on the selected card
        if let Some(card) = card_clone {
            self.detail_card = Some(card);
            self.focus = Focus::Cards;
        }
    }

    /// Move the selected card to a new column.
    pub fn move_card_to(&mut self, column_name: &str) -> Result<()> {
        let cards = self.current_cards();
        if self.card_selection >= cards.len() {
            return Ok(());
        }
        let card_id = cards[self.card_selection].id.clone();
        let card_title = cards[self.card_selection].title.clone();

        let db = db_path(&self.project_path);
        let mut store = Store::open(&db)?;
        let board = store.get_board(self.project_path.to_string_lossy().as_ref())?;

        // Find the target column ID
        let target_col = board
            .columns
            .iter()
            .find(|c| c.name == column_name)
            .ok_or_else(|| anyhow::anyhow!("Column '{}' not found", column_name))?;

        let target_col_id = target_col.id.clone();
        let target_col_name = target_col.name.clone();
        move_card_with_markdown(
            &mut store,
            &card_id,
            &target_col_id,
            &cards_dir(&self.project_path),
        )?;

        // Reload current column
        self.reload_all()?;

        // If moved out of current column, find it in new column
        let new_col_idx = self
            .columns
            .iter()
            .position(|c| c.name == target_col_name)
            .unwrap_or(0);
        self.current_column_idx = new_col_idx;
        let col_cards = &self.columns[new_col_idx].cards;
        self.card_selection = col_cards.iter().position(|c| c.id == card_id).unwrap_or(0);
        self.detail_card = col_cards.get(self.card_selection).cloned();
        self.normalize_focus_for_current_cards();

        self.set_message(format!("Moved '{}' to {}", card_title, column_name));
        Ok(())
    }

    /// Delete the selected card.
    pub fn delete_card(&mut self) -> Result<()> {
        let cards = self.current_cards();
        if self.card_selection >= cards.len() {
            return Ok(());
        }
        let card_id = cards[self.card_selection].id.clone();
        let card_title = cards[self.card_selection].title.clone();

        let db = db_path(&self.project_path);
        let mut store = Store::open(&db)?;
        delete_card_with_markdown(&mut store, &card_id, &cards_dir(&self.project_path))?;

        // Reload
        self.reload_all()?;

        self.detail_card = None;
        self.card_selection = 0;
        self.normalize_focus_for_current_cards();
        self.set_message(format!("Deleted '{}'", card_title));
        Ok(())
    }

    /// Search cards across the board.
    pub fn search(&mut self, query: &str) -> Result<()> {
        if query.is_empty() {
            self.mode = Mode::Normal;
            return Ok(());
        }
        let db = db_path(&self.project_path);
        let store = Store::open(&db)?;
        let board = store.get_board(self.project_path.to_string_lossy().as_ref())?;
        self.search_results = store.search_cards(&board.id, query)?;
        self.mode = Mode::SearchingResult;
        self.card_selection = 0;
        if let Some(card) = self.search_results.first() {
            self.detail_card = Some(card.clone());
        }
        self.normalize_focus_for_current_cards();
        Ok(())
    }

    pub fn start_editing_selected_card(&mut self) -> Result<()> {
        let cards = self.current_cards();
        let card = cards
            .get(self.card_selection)
            .ok_or_else(|| anyhow::anyhow!("No card selected to edit"))?;

        self.editor = Some(EditorState {
            card_id: card.id.clone(),
            field: EditorField::Title,
            editing_text: false,
            text_edit_original: None,
            title: card.title.clone(),
            title_cursor: card.title.chars().count(),
            description: card.description.clone(),
            description_cursor: card.description.chars().count(),
            priority: card.priority.clone(),
            labels_input: card.labels.join(", "),
            labels_cursor: card.labels.join(", ").chars().count(),
            dirty: false,
        });
        self.mode = Mode::Editing;
        self.error = None;
        Ok(())
    }

    pub fn cancel_editor(&mut self) {
        self.editor = None;
        self.mode = Mode::Normal;
        self.error = None;
    }

    pub fn editor_next_field(&mut self, forward: bool) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.editing_text {
            return;
        }

        editor.field = match (forward, &editor.field) {
            (true, EditorField::Title) => EditorField::Description,
            (true, EditorField::Description) => EditorField::Priority,
            (true, EditorField::Priority) => EditorField::Labels,
            (true, EditorField::Labels) => EditorField::Title,
            (false, EditorField::Title) => EditorField::Labels,
            (false, EditorField::Description) => EditorField::Title,
            (false, EditorField::Priority) => EditorField::Description,
            (false, EditorField::Labels) => EditorField::Priority,
        };
    }

    pub fn start_text_editor(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.field == EditorField::Priority {
            return;
        }

        editor.text_edit_original = Some(match editor.field {
            EditorField::Title => editor.title.clone(),
            EditorField::Description => editor.description.clone(),
            EditorField::Labels => editor.labels_input.clone(),
            EditorField::Priority => String::new(),
        });
        editor.editing_text = true;
    }

    pub fn finish_text_editor(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.editing_text = false;
        editor.text_edit_original = None;
    }

    pub fn cancel_text_editor(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some(original) = editor.text_edit_original.take() else {
            editor.editing_text = false;
            return;
        };

        match editor.field {
            EditorField::Title => {
                editor.title = original;
                editor.title_cursor = editor.title.chars().count();
            }
            EditorField::Description => {
                editor.description = original;
                editor.description_cursor = editor.description.chars().count();
            }
            EditorField::Labels => {
                editor.labels_input = original;
                editor.labels_cursor = editor.labels_input.chars().count();
            }
            EditorField::Priority => {}
        }
        editor.editing_text = false;
    }

    pub fn editor_insert_char(&mut self, ch: char) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if !editor.editing_text {
            return;
        }

        match editor.field {
            EditorField::Title => insert_char_at(&mut editor.title, &mut editor.title_cursor, ch),
            EditorField::Description => {
                insert_char_at(&mut editor.description, &mut editor.description_cursor, ch)
            }
            EditorField::Labels => {
                insert_char_at(&mut editor.labels_input, &mut editor.labels_cursor, ch)
            }
            EditorField::Priority => {}
        }
        editor.dirty = true;
    }

    pub fn editor_backspace(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if !editor.editing_text {
            return;
        }

        match editor.field {
            EditorField::Title => remove_char_before(&mut editor.title, &mut editor.title_cursor),
            EditorField::Description => {
                remove_char_before(&mut editor.description, &mut editor.description_cursor)
            }
            EditorField::Labels => {
                remove_char_before(&mut editor.labels_input, &mut editor.labels_cursor)
            }
            EditorField::Priority => {}
        }
        editor.dirty = true;
    }

    pub fn editor_delete(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if !editor.editing_text {
            return;
        }

        match editor.field {
            EditorField::Title => remove_char_at(&mut editor.title, editor.title_cursor),
            EditorField::Description => {
                remove_char_at(&mut editor.description, editor.description_cursor)
            }
            EditorField::Labels => remove_char_at(&mut editor.labels_input, editor.labels_cursor),
            EditorField::Priority => {}
        }
        editor.dirty = true;
    }

    pub fn editor_move_cursor(&mut self, forward: bool) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if !editor.editing_text {
            return;
        }

        match editor.field {
            EditorField::Title => {
                move_cursor_linear(&editor.title, &mut editor.title_cursor, forward)
            }
            EditorField::Description => {
                move_cursor_linear(&editor.description, &mut editor.description_cursor, forward)
            }
            EditorField::Labels => {
                move_cursor_linear(&editor.labels_input, &mut editor.labels_cursor, forward)
            }
            EditorField::Priority => {}
        }
    }

    pub fn editor_move_cursor_to_boundary(&mut self, end: bool) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if !editor.editing_text {
            return;
        }

        match editor.field {
            EditorField::Title => {
                editor.title_cursor = if end { editor.title.chars().count() } else { 0 };
            }
            EditorField::Description => {
                editor.description_cursor = if end {
                    editor.description.chars().count()
                } else {
                    0
                };
            }
            EditorField::Labels => {
                editor.labels_cursor = if end {
                    editor.labels_input.chars().count()
                } else {
                    0
                };
            }
            EditorField::Priority => {}
        }
    }

    pub fn editor_move_description_line(&mut self, down: bool) -> bool {
        let Some(editor) = &mut self.editor else {
            return false;
        };
        if !editor.editing_text
            || editor.field != EditorField::Description
            || !editor.description.contains('\n')
        {
            return false;
        }

        editor.description_cursor =
            move_cursor_vertical(&editor.description, editor.description_cursor, down);
        true
    }

    pub fn editor_cycle_priority(&mut self, forward: bool) {
        let Some(editor) = &mut self.editor else {
            return;
        };

        if editor.field != EditorField::Priority {
            return;
        }

        editor.priority = match (forward, editor.priority.clone()) {
            (true, Priority::Backlog) => Priority::Low,
            (true, Priority::Low) => Priority::Medium,
            (true, Priority::Medium) => Priority::High,
            (true, Priority::High) => Priority::Urgent,
            (true, Priority::Urgent) => Priority::Backlog,
            (false, Priority::Backlog) => Priority::Urgent,
            (false, Priority::Low) => Priority::Backlog,
            (false, Priority::Medium) => Priority::Low,
            (false, Priority::High) => Priority::Medium,
            (false, Priority::Urgent) => Priority::High,
        };
        editor.dirty = true;
    }

    pub fn save_editor(&mut self) -> Result<()> {
        let editor = self
            .editor
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No editor is open"))?;

        let title = editor.title.trim().to_string();
        if title.is_empty() {
            return Err(anyhow::anyhow!("Title cannot be empty"));
        }

        let db = db_path(&self.project_path);
        let mut store = Store::open(&db)?;
        let updated = update_card_with_markdown(
            &mut store,
            &editor.card_id,
            CardPatch {
                title: Some(title),
                description: Some(editor.description),
                column_id: None,
                priority: Some(editor.priority),
                labels: Some(parse_labels_input(&editor.labels_input)),
            },
            &cards_dir(&self.project_path),
        )?;

        self.reload_all()?;
        self.select_card_by_id(&updated.id);
        self.mode = Mode::Normal;
        self.editor = None;
        self.error = None;
        self.set_message(format!("Updated '{}'", updated.title));
        Ok(())
    }

    fn select_card_by_id(&mut self, card_id: &str) {
        for (column_idx, column) in self.columns.iter().enumerate() {
            if let Some(card_idx) = column.cards.iter().position(|card| card.id == card_id) {
                self.current_column_idx = column_idx;
                self.card_selection = card_idx;
                self.detail_card = column.cards.get(card_idx).cloned();
                self.normalize_focus_for_current_cards();
                return;
            }
        }
        self.normalize_focus_for_current_cards();
    }

    fn selected_card_id(&self) -> Option<String> {
        self.detail_card
            .as_ref()
            .map(|card| card.id.clone())
            .or_else(|| {
                self.current_cards()
                    .get(self.card_selection)
                    .map(|card| card.id.clone())
            })
    }

    pub fn normalize_focus_for_current_cards(&mut self) {
        if self.columns.is_empty() {
            self.current_column_idx = 0;
            self.card_selection = 0;
            self.detail_card = None;
            self.focus = Focus::Columns;
            return;
        }

        self.current_column_idx = self.current_column_idx.min(self.columns.len() - 1);
        let card_count = self.current_cards().len();
        if card_count == 0 {
            self.card_selection = 0;
            self.detail_card = None;
            if matches!(self.focus, Focus::Cards | Focus::Detail) {
                self.focus = Focus::Columns;
            }
        } else {
            self.card_selection = self.card_selection.min(card_count - 1);
        }
    }

    /// Display a temporary message.
    pub fn set_message(&mut self, msg: String) {
        self.message = Some(msg);
        self.message_time = std::time::Instant::now();
    }

    /// Check if a message has expired (3 seconds).
    pub fn message_expired(&self) -> bool {
        self.message_time.elapsed().as_secs() > 3
    }
}

fn parse_labels_input(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn insert_char_at(text: &mut String, cursor: &mut usize, ch: char) {
    let byte_idx = char_to_byte_idx(text, *cursor);
    text.insert(byte_idx, ch);
    *cursor += 1;
}

fn remove_char_before(text: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    *cursor -= 1;
    remove_char_at(text, *cursor);
}

fn remove_char_at(text: &mut String, cursor: usize) {
    let char_count = text.chars().count();
    if cursor >= char_count {
        return;
    }
    let start = char_to_byte_idx(text, cursor);
    let end = char_to_byte_idx(text, cursor + 1);
    text.replace_range(start..end, "");
}

fn move_cursor_linear(text: &str, cursor: &mut usize, forward: bool) {
    let char_count = text.chars().count();
    if forward {
        *cursor = (*cursor + 1).min(char_count);
    } else {
        *cursor = cursor.saturating_sub(1);
    }
}

fn move_cursor_vertical(text: &str, cursor: usize, down: bool) -> usize {
    let mut line_start = 0;
    let mut column = cursor;
    for (idx, ch) in text.chars().enumerate() {
        if idx >= cursor {
            break;
        }
        if ch == '\n' {
            line_start = idx + 1;
            column = cursor - line_start;
        }
    }

    if down {
        let current_line_end = text
            .chars()
            .enumerate()
            .skip(cursor)
            .find_map(|(idx, ch)| (ch == '\n').then_some(idx))
            .unwrap_or_else(|| text.chars().count());
        if current_line_end == text.chars().count() {
            return cursor;
        }
        let next_start = current_line_end + 1;
        let next_len = line_len_from(text, next_start);
        next_start + column.min(next_len)
    } else {
        if line_start == 0 {
            return cursor;
        }
        let previous_end = line_start - 1;
        let previous_start = text
            .chars()
            .take(previous_end)
            .enumerate()
            .filter_map(|(idx, ch)| (ch == '\n').then_some(idx + 1))
            .last()
            .unwrap_or(0);
        let previous_len = previous_end - previous_start;
        previous_start + column.min(previous_len)
    }
}

fn line_len_from(text: &str, start: usize) -> usize {
    text.chars()
        .skip(start)
        .take_while(|ch| *ch != '\n')
        .count()
}

fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

/// Run the TUI. Returns when the user quits.
pub fn run(project_path: PathBuf) -> Result<()> {
    let mut app = App::new(project_path)?;
    let mut terminal = ratatui::init();
    let mut last_frame_size = ratatui::layout::Size {
        width: terminal.size()?.width,
        height: terminal.size()?.height,
    };
    let mut last_refresh = std::time::Instant::now();

    while app.running {
        // Handle events
        if event::poll(std::time::Duration::from_millis(16))? {
            if let CEvent::Key(key) = event::read()? {
                // Ignore key release events
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if let Err(e) = events::handle_key(key, &mut app) {
                    app.error = Some(e.to_string());
                    app.message_time = std::time::Instant::now();
                }
            }
        }

        // Clear expired messages
        if app.message_expired() {
            app.message = None;
            app.error = None;
        }

        if last_refresh.elapsed() >= std::time::Duration::from_secs(1)
            && matches!(
                app.mode,
                Mode::Normal | Mode::SearchingResult | Mode::Moving | Mode::Searching
            )
        {
            if let Err(e) = app.reload_all() {
                app.error = Some(e.to_string());
                app.message_time = std::time::Instant::now();
            }
            last_refresh = std::time::Instant::now();
        }

        // Check terminal resize
        let size = terminal.size()?;
        if size.width != last_frame_size.width || size.height != last_frame_size.height {
            let rect = ratatui::layout::Rect::new(0, 0, size.width, size.height);
            terminal.resize(rect)?;
            last_frame_size = size;
        }

        // Render
        terminal.draw(|frame| {
            render::render(frame, &app);
        })?;
    }

    ratatui::restore();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kanban::config::{cards_dir, db_path};
    use crate::kanban::init::init_board;
    use crate::persistence::create_card_with_markdown;
    use std::fs;

    struct Fixture {
        project: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let project =
                std::env::temp_dir().join(format!("kanban_tui_editor_{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&project).unwrap();
            init_board(&project).unwrap();
            Self { project }
        }

        fn create_card(&self, id: &str) -> Card {
            let db = db_path(&self.project);
            let mut store = Store::open(&db).unwrap();
            let board = store
                .get_board(self.project.to_string_lossy().as_ref())
                .unwrap();
            let column = board
                .columns
                .iter()
                .find(|column| column.name == "backlog")
                .unwrap();
            let mut card = Card::new(
                &board.id,
                &column.id,
                "Original title",
                "Original description",
                Priority::Medium,
                vec!["old".to_string()],
                PathBuf::from(format!("{id}.md")),
            );
            card.id = id.to_string();
            create_card_with_markdown(&mut store, &card, &cards_dir(&self.project)).unwrap();
            card
        }
    }

    #[test]
    fn app_new_reports_uninitialized_project_before_opening_database() {
        let project =
            std::env::temp_dir().join(format!("kanban_tui_uninitialized_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&project).unwrap();

        let err = match App::new(project.clone()) {
            Ok(_) => panic!("uninitialized project should not open in TUI"),
            Err(err) => err,
        };

        let message = err.to_string();
        assert!(message.contains("No kanban board found"));
        assert!(message.contains(project.to_string_lossy().as_ref()));
        assert!(!message.contains("Database temporarily locked"));

        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn app_starts_on_columns_when_initial_column_is_empty() {
        let fixture = Fixture::new();
        let app = App::new(fixture.project.clone()).unwrap();

        assert_eq!(app.current_cards().len(), 0);
        assert_eq!(app.focus, Focus::Columns);
    }

    #[test]
    fn column_navigation_keeps_focus_on_empty_columns() {
        let fixture = Fixture::new();
        fixture.create_card("edit-non-empty");
        let mut app = App::new(fixture.project.clone()).unwrap();
        app.current_column_idx = app
            .columns
            .iter()
            .position(|column| column.name == "backlog")
            .unwrap();
        app.focus = Focus::Cards;

        app.column_next(true);

        assert_eq!(app.current_cards().len(), 0);
        assert_eq!(app.focus, Focus::Columns);
    }

    #[test]
    fn reload_all_picks_up_external_card_changes() {
        let fixture = Fixture::new();
        let mut app = App::new(fixture.project.clone()).unwrap();
        assert_eq!(app.all_cards.len(), 0);

        fixture.create_card("external-card");
        app.reload_all().unwrap();

        assert_eq!(app.all_cards.len(), 1);
        assert!(app
            .columns
            .iter()
            .any(|column| { column.cards.iter().any(|card| card.id == "external-card") }));
    }

    #[test]
    fn reload_all_preserves_empty_detail_when_only_list_selection_exists() {
        let fixture = Fixture::new();
        fixture.create_card("selected-but-no-detail");
        let mut app = App::new(fixture.project.clone()).unwrap();
        assert!(app.detail_card.is_none());

        app.reload_all().unwrap();

        assert!(app.detail_card.is_none());
        assert_eq!(
            app.current_cards()[app.card_selection].id,
            "selected-but-no-detail"
        );
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.project);
        }
    }

    #[test]
    fn start_editing_selected_card_copies_card_fields() {
        let fixture = Fixture::new();
        let card = fixture.create_card("edit-copy");
        let mut app = App::new(fixture.project.clone()).unwrap();
        app.card_selection = app
            .current_cards()
            .iter()
            .position(|c| c.id == card.id)
            .unwrap();

        app.start_editing_selected_card().unwrap();

        let editor = app.editor.as_ref().unwrap();
        assert_eq!(app.mode, Mode::Editing);
        assert_eq!(editor.card_id, "edit-copy");
        assert_eq!(editor.title, "Original title");
        assert_eq!(editor.description, "Original description");
        assert_eq!(editor.priority, Priority::Medium);
        assert_eq!(editor.labels_input, "old");
    }

    #[test]
    fn save_editor_updates_sqlite_and_markdown_export() {
        let fixture = Fixture::new();
        let card = fixture.create_card("edit-save");
        let mut app = App::new(fixture.project.clone()).unwrap();
        app.card_selection = app
            .current_cards()
            .iter()
            .position(|c| c.id == card.id)
            .unwrap();
        app.start_editing_selected_card().unwrap();

        let editor = app.editor.as_mut().unwrap();
        editor.title = "Updated title".to_string();
        editor.description = "Updated description".to_string();
        editor.priority = Priority::High;
        editor.labels_input = "new, ui".to_string();

        app.save_editor().unwrap();

        let store = Store::open(&db_path(&fixture.project)).unwrap();
        let stored = store.get_card("edit-save").unwrap();
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.editor.is_none());
        assert_eq!(stored.title, "Updated title");
        assert_eq!(stored.description, "Updated description");
        assert_eq!(stored.priority, Priority::High);
        assert_eq!(stored.labels, vec!["new".to_string(), "ui".to_string()]);

        let export = fs::read_to_string(cards_dir(&fixture.project).join("edit-save.md")).unwrap();
        assert!(export.contains("title: \"Updated title\""));
        assert!(export.contains("Updated description"));
    }

    #[test]
    fn save_editor_rejects_empty_title_without_persisting() {
        let fixture = Fixture::new();
        let card = fixture.create_card("edit-empty-title");
        let mut app = App::new(fixture.project.clone()).unwrap();
        app.card_selection = app
            .current_cards()
            .iter()
            .position(|c| c.id == card.id)
            .unwrap();
        app.start_editing_selected_card().unwrap();
        app.editor.as_mut().unwrap().title = "   ".to_string();

        let err = app.save_editor().unwrap_err();

        let store = Store::open(&db_path(&fixture.project)).unwrap();
        let stored = store.get_card("edit-empty-title").unwrap();
        assert_eq!(app.mode, Mode::Editing);
        assert!(app.editor.is_some());
        assert!(err.to_string().contains("Title cannot be empty"));
        assert_eq!(stored.title, "Original title");
    }
}
