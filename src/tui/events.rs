use crossterm::event::{KeyCode, KeyModifiers};

use super::app::{App, EditorField, Focus, Mode};

/// Handle a key event and mutate the app state.
/// Returns Ok(()) on success, Err on error (e.g., DB failure).
pub fn handle_key(key: crossterm::event::KeyEvent, app: &mut App) -> anyhow::Result<()> {
    let key_code = key.code;
    let modifiers = key.modifiers;

    if app.mode == Mode::Editing {
        let editing_text = app
            .editor
            .as_ref()
            .is_some_and(|editor| editor.editing_text);

        if editing_text {
            match key_code {
                KeyCode::Esc => {
                    app.cancel_text_editor();
                }
                KeyCode::Char('s') if modifiers == KeyModifiers::CONTROL => {
                    app.finish_text_editor();
                }
                KeyCode::Left => {
                    app.editor_move_cursor(false);
                }
                KeyCode::Right => {
                    app.editor_move_cursor(true);
                }
                KeyCode::Down if app.editor_move_description_line(true) => {}
                KeyCode::Up if app.editor_move_description_line(false) => {}
                KeyCode::Backspace => {
                    app.editor_backspace();
                }
                KeyCode::Delete => {
                    app.editor_delete();
                }
                KeyCode::Home => {
                    app.editor_move_cursor_to_boundary(false);
                }
                KeyCode::End => {
                    app.editor_move_cursor_to_boundary(true);
                }
                KeyCode::Enter => {
                    if app
                        .editor
                        .as_ref()
                        .is_some_and(|editor| editor.field == EditorField::Description)
                    {
                        app.editor_insert_char('\n');
                    } else {
                        app.finish_text_editor();
                    }
                }
                KeyCode::Char(c) if is_text_input_modifier(modifiers) => {
                    app.editor_insert_char(c);
                }
                _ => {}
            }
            return Ok(());
        }

        match key_code {
            KeyCode::Esc => {
                app.cancel_editor();
            }
            KeyCode::Char('s') if modifiers == KeyModifiers::CONTROL => {
                app.save_editor()?;
            }
            KeyCode::Tab => {
                app.editor_next_field(true);
            }
            KeyCode::BackTab => {
                app.editor_next_field(false);
            }
            KeyCode::Down => {
                app.editor_next_field(true);
            }
            KeyCode::Up => {
                app.editor_next_field(false);
            }
            KeyCode::Left => {
                if app
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.field == EditorField::Priority)
                {
                    app.editor_cycle_priority(false);
                } else {
                    app.editor_move_cursor(false);
                }
            }
            KeyCode::Right => {
                if app
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.field == EditorField::Priority)
                {
                    app.editor_cycle_priority(true);
                } else {
                    app.editor_move_cursor(true);
                }
            }
            KeyCode::Backspace => {}
            KeyCode::Delete => {}
            KeyCode::Enter => {
                app.start_text_editor();
            }
            KeyCode::Char('e') if is_text_input_modifier(modifiers) => {
                app.start_text_editor();
            }
            _ => {}
        }
        return Ok(());
    }

    // If in search mode, handle search input
    if app.mode == Mode::Searching {
        match key_code {
            KeyCode::Enter => {
                let query = app.search_query.clone();
                app.search(&query)?;
                app.focus = Focus::Cards;
                app.normalize_focus_for_current_cards();
            }
            KeyCode::Esc => {
                app.mode = Mode::Normal;
                app.search_query.clear();
            }
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            KeyCode::Char(c) if is_text_input_modifier(modifiers) => {
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
                app.normalize_focus_for_current_cards();
            }
            KeyCode::Char('t') | KeyCode::Char('1') => {
                app.move_card_to("todo")?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
                app.normalize_focus_for_current_cards();
            }
            KeyCode::Char('i') | KeyCode::Char('2') => {
                app.move_card_to("in_progress")?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
                app.normalize_focus_for_current_cards();
            }
            KeyCode::Char('r') | KeyCode::Char('3') => {
                app.move_card_to("review")?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
                app.normalize_focus_for_current_cards();
            }
            KeyCode::Char('d') => {
                app.move_card_to("done")?;
                app.mode = Mode::Normal;
                app.focus = Focus::Cards;
                app.normalize_focus_for_current_cards();
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
            app.column_next(false);
        }
        KeyCode::Down | KeyCode::Char('j') if app.focus == Focus::Columns => {
            app.column_next(true);
        }

        // Navigate cards
        KeyCode::Up | KeyCode::Char('k') => {
            app.card_nav(false);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.card_nav(true);
        }

        // Enter: focus the next actionable panel.
        KeyCode::Enter if app.focus == Focus::Columns && !app.current_cards().is_empty() => {
            app.focus = Focus::Cards;
            app.card_selection = 0;
            app.detail_card = None;
        }
        KeyCode::Enter if app.focus == Focus::Cards => {
            app.start_editing_selected_card()?;
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
            app.normalize_focus_for_current_cards();
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

        // e: edit selected card fields
        KeyCode::Char('e') => {
            app.start_editing_selected_card()?;
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

fn is_text_input_modifier(modifiers: KeyModifiers) -> bool {
    !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
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
            editor: None,
            message: None,
            message_time: std::time::Instant::now(),
        }
    }

    fn test_app_with_columns() -> App {
        let mut app = test_app_with_card();
        app.focus = Focus::Columns;
        app.columns = vec![
            ColumnView {
                name: "backlog".to_string(),
                cards: vec![],
            },
            ColumnView {
                name: "todo".to_string(),
                cards: vec![],
            },
            ColumnView {
                name: "done".to_string(),
                cards: vec![],
            },
        ];
        app.current_column_idx = 1;
        app.card_selection = 0;
        app.detail_card = None;
        app
    }

    #[test]
    fn column_arrow_keys_match_vertical_direction() {
        let mut app = test_app_with_columns();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        assert_eq!(app.current_column_idx, 0);

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        assert_eq!(app.current_column_idx, 1);
    }

    #[test]
    fn right_arrow_from_selected_card_shows_detail() {
        let mut app = test_app_with_card();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert_eq!(app.focus, Focus::Detail);
        assert_eq!(
            app.detail_card.as_ref().map(|card| card.id.as_str()),
            Some("test-card")
        );
    }

    #[test]
    fn enter_from_selected_card_opens_editor() {
        let mut app = test_app_with_card();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert_eq!(app.mode, Mode::Editing);
        assert_eq!(app.editor.as_ref().unwrap().card_id, "test-card");
    }

    #[test]
    fn e_key_opens_card_editor() {
        let mut app = test_app_with_card();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert_eq!(app.mode, Mode::Editing);
        assert_eq!(app.editor.as_ref().unwrap().title, "Test card");
    }

    #[test]
    fn readme_markdown_export_docs_do_not_advertise_direct_markdown_editing() {
        let readme = include_str!("../../README.md");

        assert!(
            readme.contains("does not edit markdown files directly"),
            "README should state that TUI editing does not edit markdown files directly"
        );
        assert!(
            readme
                .contains("Direct edits to these markdown files are not imported back into SQLite"),
            "README should keep markdown exports separate from authoritative SQLite edits"
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

    #[test]
    fn editor_escape_cancels_without_changing_title() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();
        app.editor.as_mut().unwrap().title = "Changed".to_string();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert_eq!(app.mode, Mode::Normal);
        assert!(app.editor.is_none());
        assert_eq!(app.current_cards()[0].title, "Test card");
    }

    #[test]
    fn editor_text_input_updates_active_field() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert_eq!(app.editor.as_ref().unwrap().title, "Test card!");
    }

    #[test]
    fn editor_field_list_ignores_text_until_text_editor_is_opened() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        let editor = app.editor.as_ref().unwrap();
        assert!(!editor.editing_text);
        assert_eq!(editor.title, "Test card");
    }

    #[test]
    fn e_key_opens_selected_string_field_text_editor() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert!(app.editor.as_ref().unwrap().editing_text);
    }

    #[test]
    fn escape_in_text_editor_cancels_only_the_active_field_edit() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        let editor = app.editor.as_ref().unwrap();
        assert_eq!(app.mode, Mode::Editing);
        assert!(!editor.editing_text);
        assert_eq!(editor.title, "Test card");
    }

    #[test]
    fn editor_left_right_move_cursor_and_insert_within_title() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        for _ in 0..4 {
            handle_key(
                crossterm::event::KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
                &mut app,
            )
            .unwrap();
        }
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT),
            &mut app,
        )
        .unwrap();

        let editor = app.editor.as_ref().unwrap();
        assert_eq!(editor.title, "Test Xcard");
    }

    #[test]
    fn editor_delete_removes_character_at_cursor() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        for _ in 0..4 {
            handle_key(
                crossterm::event::KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
                &mut app,
            )
            .unwrap();
        }
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert_eq!(app.editor.as_ref().unwrap().title, "Test ard");
    }

    #[test]
    fn editor_up_down_move_cursor_between_description_lines() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();
        {
            let editor = app.editor.as_mut().unwrap();
            editor.field = EditorField::Description;
            editor.description = "abc\ndefgh".to_string();
        }
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT),
            &mut app,
        )
        .unwrap();

        assert_eq!(app.editor.as_ref().unwrap().description, "abc\ndeXfgh");
    }

    #[test]
    fn editor_accepts_shifted_text_input_in_editable_fields() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut app,
        )
        .unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut app,
        )
        .unwrap();

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();
        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::SHIFT),
            &mut app,
        )
        .unwrap();

        let editor = app.editor.as_ref().unwrap();
        assert_eq!(editor.title, "Test cardX");
        assert_eq!(editor.description, "DescriptionY");
        assert_eq!(editor.labels_input, "Z");
    }

    #[test]
    fn editor_tab_and_arrows_change_fields_and_priority() {
        let mut app = test_app_with_card();
        app.start_editing_selected_card().unwrap();

        for _ in 0..2 {
            handle_key(
                crossterm::event::KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
                &mut app,
            )
            .unwrap();
        }
        assert_eq!(
            app.editor.as_ref().unwrap().field,
            super::super::app::EditorField::Priority
        );

        handle_key(
            crossterm::event::KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut app,
        )
        .unwrap();

        assert_eq!(app.editor.as_ref().unwrap().priority, Priority::High);
    }
}
