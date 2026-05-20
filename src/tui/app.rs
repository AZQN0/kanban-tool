#![allow(dead_code)]

use anyhow::{Context, Result};
use crossterm::event::{self, Event as CEvent, KeyEventKind};
use std::path::PathBuf;

use crate::board::card::Card;
use crate::board::store::{CardSort, Store};
use crate::kanban::config::{cards_dir, db_path};
use crate::persistence::{delete_card_with_markdown, move_card_with_markdown};

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
    ProjectPicker,
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

    // Project picker
    pub all_projects: Vec<(PathBuf, String)>,
    pub project_picker_idx: usize,

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

        Ok(App {
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
            all_projects: Vec::new(),
            project_picker_idx: 0,
            message: None,
            message_time: std::time::Instant::now(),
        })
    }

    /// Reload the current column's cards from the store.
    pub fn reload_current_column(&mut self) -> Result<()> {
        let db = db_path(&self.project_path);
        let store = Store::open(&db)?;
        let snapshot = store.load_board_snapshot(
            self.project_path.to_string_lossy().as_ref(),
            CardSort::Priority,
        )?;
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
        if !self.columns.is_empty() {
            self.current_column_idx = self.current_column_idx.min(self.columns.len() - 1);
        }
        Ok(())
    }

    /// Reload all columns.
    pub fn reload_all(&mut self) -> Result<()> {
        let db = db_path(&self.project_path);
        let store = Store::open(&db)?;
        let snapshot = store.load_board_snapshot(
            self.project_path.to_string_lossy().as_ref(),
            CardSort::Priority,
        )?;

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
        if !self.columns.is_empty() {
            self.current_column_idx = self.current_column_idx.min(self.columns.len() - 1);
        }
        Ok(())
    }

    /// Get the cards for the current column (or search results if searching).
    pub fn current_cards(&self) -> &[Card] {
        match self.mode {
            Mode::SearchingResult => &self.search_results,
            _ => &self.columns[self.current_column_idx].cards,
        }
    }

    /// Move focus to the next/previous panel.
    pub fn focus_next(&mut self, forward: bool) {
        self.mode = Mode::Normal;
        self.detail_card = None;
        match (forward, self.focus.clone()) {
            (true, Focus::Columns) => self.focus = Focus::Cards,
            (true, Focus::Cards) => self.focus = Focus::Detail,
            (true, Focus::Detail) => self.focus = Focus::Columns,
            (false, Focus::Columns) => self.focus = Focus::Detail,
            (false, Focus::Cards) => self.focus = Focus::Columns,
            (false, Focus::Detail) => self.focus = Focus::Cards,
        }
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
    }

    /// Navigate within the current cards list.
    pub fn card_nav(&mut self, forward: bool) {
        let cards = self.current_cards();
        if cards.is_empty() {
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
        Ok(())
    }

    /// Load all projects from the database.
    pub fn load_projects(&mut self) -> Result<()> {
        let db = db_path(&self.project_path);
        let store = Store::open(&db)?;
        let boards = store.list_boards()?;
        self.all_projects = boards
            .iter()
            .map(|b| (PathBuf::from(&b.project_path), b.name.clone()))
            .collect();
        Ok(())
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

/// Run the TUI. Returns when the user quits.
pub fn run(project_path: PathBuf) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut app = match App::new(project_path.clone()) {
        Ok(a) => a,
        Err(e) => {
            terminal.clear()?;
            eprintln!("Error: {}", e);
            ratatui::restore();
            return Err(e);
        }
    };

    let mut last_frame_size = ratatui::layout::Size {
        width: terminal.size()?.width,
        height: terminal.size()?.height,
    };

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
