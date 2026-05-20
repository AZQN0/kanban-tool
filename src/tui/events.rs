use crossterm::event::{KeyCode, KeyModifiers};

use super::app::{App, Focus, Mode};

/// Handle a key event and mutate the app state.
/// Returns Ok(()) on success, Err on error (e.g., DB failure).
pub fn handle_key(key: crossterm::event::KeyEvent, app: &mut App) -> anyhow::Result<()> {
    let key_code = key.code;
    let modifiers = key.modifiers;

    // If in search mode, handle search input
    if app.mode == Mode::Searching {
        match key_code {
            KeyCode::Enter => {
                let query = app.search_query.clone();
                app.search(&query)?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
            }
            KeyCode::Esc => {
                app.mode = Mode::Normal;
                app.search_query.clear();
            }
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            KeyCode::Char(c) if modifiers == KeyModifiers::NONE => {
                app.search_query.push(c);
            }
            _ => {}
        }
        return Ok(());
    }

    // If in move mode, handle column selection
    if app.mode == Mode::Moving {
        match key_code {
            KeyCode::Char('b') | KeyCode::Char('0') => {
                app.move_card_to("backlog")?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
            }
            KeyCode::Char('t') | KeyCode::Char('1') => {
                app.move_card_to("todo")?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
            }
            KeyCode::Char('i') | KeyCode::Char('2') => {
                app.move_card_to("in_progress")?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
            }
            KeyCode::Char('r') | KeyCode::Char('3') => {
                app.move_card_to("review")?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
            }
            KeyCode::Char('d') => {
                app.move_card_to("done")?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
            }
            KeyCode::Esc => {
                app.mode = Mode::Normal;
            }
            _ => {}
        }
        return Ok(());
    }

    // Normal mode key handling
    match key_code {
        // Quit
        KeyCode::Char('q') => {
            app.running = false;
        }

        // Move focus between panels
        KeyCode::Right | KeyCode::Char('l') => {
            app.focus_next(true);
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.focus_next(false);
        }

        // Navigate columns
        KeyCode::Up | KeyCode::Char('k') if app.focus == Focus::Columns => {
            app.column_next(true);
        }
        KeyCode::Down | KeyCode::Char('j') if app.focus == Focus::Columns => {
            app.column_next(false);
        }

        // Navigate cards
        KeyCode::Up | KeyCode::Char('k') => {
            app.card_nav(false);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.card_nav(true);
        }

        // Enter: focus detail view on selected card
        KeyCode::Enter if app.focus == Focus::Cards || app.focus == Focus::Columns => {
            let cards = app.current_cards();
            if !cards.is_empty() && app.card_selection < cards.len() {
                app.detail_card = Some(cards[app.card_selection].clone());
                app.focus = Focus::Cards;
            }
        }
        // Enter: unfocus detail view
        KeyCode::Enter if app.focus == Focus::Detail => {
            app.detail_card = None;
            app.focus = Focus::Cards;
        }

        // Escape: cancel current mode
        KeyCode::Esc => {
            app.detail_card = None;
            app.mode = Mode::Normal;
            app.focus = Focus::Cards;
        }

        // m: start moving selected card
        KeyCode::Char('m') => {
            app.mode = Mode::Moving;
            app.set_message("Move to: [b]acklog [t]odo [i]n_progress [r]eview [d]one".to_string());
        }

        // d: delete selected card (confirm with y/n)
        KeyCode::Char('D') => {
            app.delete_card()?;
        }

        // /: start search
        KeyCode::Char('/') => {
            app.search_query.clear();
            app.mode = Mode::Searching;
        }

        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::board::card::{Card, Priority};
    use crate::tui::app::ColumnView;

    fn test_app_with_card() -> App {
        let mut card = Card::new(
            "board-1",
            "todo",
            "Test card",
            "Description",
            Priority::Medium,
            vec![],
            PathBuf::from("test-card.md"),
        );
        card.id = "test-card".to_string();

        App {
            running: true,
            focus: Focus::Cards,
            mode: Mode::Normal,
            error: None,
            project_path: PathBuf::from("/tmp/kanban-no-edit-test"),
            board_name: "Test Board".to_string(),
            columns: vec![ColumnView {
                name: "todo".to_string(),
                cards: vec![card.clone()],
            }],
            all_cards: vec![card],
            current_column_idx: 0,
            card_selection: 0,
            detail_card: None,
            search_query: String::new(),
            search_results: vec![],
            message: None,
            message_time: std::time::Instant::now(),
        }
    }

    #[test]
    fn e_key_does_not_open_markdown_export_editor() {
        let mut app = test_app_with_card();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert_ne!(
            app.message.as_deref(),
            Some("Editor opened. Press any key to continue...")
        );
    }

    #[test]
    fn readme_markdown_export_docs_do_not_advertise_tui_editing() {
        let readme = include_str!("../../README.md");

        assert!(
            !readme.contains("edit cards through the CLI, TUI"),
            "README still advertises TUI editing of authoritative card data"
        );
    }

    #[test]
    fn p_key_does_not_open_project_picker() {
        let mut app = test_app_with_card();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('P'), KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert_eq!(app.mode, Mode::Normal);
    }
}
