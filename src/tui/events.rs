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
            KeyCode::Char(c) => {
                if modifiers == KeyModifiers::NONE {
                    app.search_query.push(c);
                }
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

    // If in project picker mode
    if app.mode == Mode::ProjectPicker {
        match key_code {
            KeyCode::Enter => {
                if !app.all_projects.is_empty() && app.project_picker_idx < app.all_projects.len() {
                    let (path, _name) = &app.all_projects[app.project_picker_idx];
                    if path != &app.project_path {
                        // Reload the app with new project path
                        let new_app = super::app::App::new(path.clone())?;
                        *app = new_app;
                    }
                }
                app.mode = super::app::Mode::Normal;
            }
            KeyCode::Char('p') if app.project_picker_idx > 0 => {
                app.project_picker_idx -= 1;
            }
            KeyCode::Char('n') if app.project_picker_idx + 1 < app.all_projects.len() => {
                app.project_picker_idx += 1;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
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

        // e: edit selected card in $EDITOR
        KeyCode::Char('e') => {
            app.edit_card()?;
            app.set_message("Editor opened. Press any key to continue...".to_string());
            // Note: we can't actually wait for editor exit here without blocking.
            // The reload will happen after the terminal re-renders.
            // We'll reload on next keypress.
            app.reload_all().ok();
        }

        // d: delete selected card (confirm with y/n)
        KeyCode::Char('D') => {
            app.delete_card()?;
        }

        // P: project picker
        KeyCode::Char('P') => {
            app.load_projects()?;
            app.project_picker_idx = 0;
            app.mode = Mode::ProjectPicker;
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
