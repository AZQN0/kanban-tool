use crate::board::card::{Card, Priority};

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use super::app::{App, Focus, Mode};

/// Render the full TUI.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Top status bar
            Constraint::Min(1),     // Main area
            Constraint::Length(3),  // Bottom status bar
        ])
        .split(frame.area());

    render_top_statusbar(frame, app, chunks[0]);
    render_main_area(frame, app, chunks[1]);
    render_bottom_statusbar(frame, app, chunks[2]);
}

/// Top status bar showing project name and card count.
fn render_top_statusbar(frame: &mut Frame, app: &App, area: Rect) {
    let project_name = &app.board_name;
    let total = app.all_cards.len();

    let text = match app.mode {
        Mode::Searching => format!(" 🔍 Searching: {} | Project: {} | Cards: {total}", app.search_query, project_name),
        Mode::ProjectPicker => format!(" 📋 Projects ({}/{}): Project: {}", app.project_picker_idx + 1, app.all_projects.len(), project_name),
        _ => format!(" 📋 Project: {} | Cards: {total}", project_name),
    };

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan)))
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}

/// Main area with 3 panels: columns, cards, detail.
fn render_main_area(frame: &mut Frame, app: &App, area: Rect) {
    // Handle project picker overlay
    if app.mode == Mode::ProjectPicker {
        render_project_picker(frame, app, area);
        return;
    }

    // Handle move popup
    if app.mode == Mode::Moving {
        render_move_popup(frame, app, area);
        return;
    }

    // Handle search input overlay
    if app.mode == Mode::Searching {
        render_search_input(frame, app, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),  // Columns panel
            Constraint::Min(20),     // Cards panel
            Constraint::Length(40),  // Detail panel
        ])
        .split(area);

    render_columns(frame, app, chunks[0]);
    render_cards(frame, app, chunks[1]);
    render_detail(frame, app, chunks[2]);
}

/// Render the columns panel (left).
fn render_columns(frame: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = Vec::new();

    for (i, col) in app.columns.iter().enumerate() {
        let count = col.cards.len();
        let name = col.name.clone();

        let is_selected = i == app.current_column_idx;
        let label = if is_selected {
            format!("▶ {}", name)
        } else {
            format!("  {}", name)
        };

        let _style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let full_line = if is_selected {
            format!("\u{25b6} {} ({})", label, count)
        } else {
            format!("  {} ({})", label, count)
        };
        items.push(ListItem::new(full_line));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Columns ")
                .style(Style::default().fg(Color::Gray))
        )
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    frame.render_widget(list, area);
}

/// Render the cards panel (center).
fn render_cards(frame: &mut Frame, app: &App, area: Rect) {
    let cards = app.current_cards();

    // Get the current column name for the title
    let col_name = if app.mode == Mode::SearchingResult {
        "Search Results".to_string()
    } else {
        app.columns.get(app.current_column_idx)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "?".to_string())
    };

    let mut items: Vec<ListItem> = Vec::new();

    for (i, card) in cards.iter().enumerate() {
        let priority_indicator = priority_indicator(&card.priority);
        let title = truncate(&card.title, area.width as usize - 5);

        let is_selected = i == app.card_selection;
        let _style = if is_selected {
            Style::default().fg(Color::White).add_modifier(Modifier::REVERSED)
        } else if card.priority == Priority::Urgent {
            Style::default().fg(Color::Red)
        } else if card.priority == Priority::High {
            Style::default().fg(Color::Magenta)
        } else {
            Style::default().fg(Color::White)
        };

        let id_short: String = card.id.chars().take(8).collect();
        items.push(ListItem::new(format!("{} {} {}", priority_indicator, id_short, title)));
    }

    if items.is_empty() {
        items.push(ListItem::new(" (empty) "));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", col_name))
                .style(Style::default().fg(Color::Gray))
        )
        .highlight_style(Style::default().fg(Color::White).add_modifier(Modifier::REVERSED));

    frame.render_widget(list, area);
}

/// Render the card detail panel (right).
fn render_detail(frame: &mut Frame, app: &App, area: Rect) {
    let text = if let Some(ref card) = app.detail_card {
        render_card_detail(card)
    } else {
        vec![Line::from(" No card selected. ")]
    };

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Detail ")
                .style(Style::default().fg(Color::Gray))
        )
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// Render the bottom status bar with key bindings.
fn render_bottom_statusbar(frame: &mut Frame, app: &App, area: Rect) {
    let focus_indicator = match app.focus {
        Focus::Columns => "[C]olumns",
        Focus::Cards => "[C]ards",
        Focus::Detail => "[D]etail",
    };

    let text = if let Some(error) = &app.error {
        format!(" {} | ERROR: {}", focus_indicator, error)
    } else if app.message.is_some() {
        format!(" {} | ↑↓ Navigate | Enter Focus | m Move | D Delete | P Project | / Search | q Quit | {}", focus_indicator, app.message.as_ref().unwrap_or(&String::new()))
    } else {
        format!(" {} | ↑↓ Navigate | Enter Focus | m Move | D Delete | P Project | / Search | q Quit", focus_indicator)
    };

    let style = if app.error.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).style(style))
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}

/// Render the project picker overlay.
fn render_project_picker(frame: &mut Frame, app: &App, area: Rect) {
    // Dim background
    let bg = Paragraph::new("").style(Style::default().bg(Color::Black));
    frame.render_widget(bg, area);

    let picker_area = center_rect(area, 50, 10);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Switch Project ")
        .style(Style::default().fg(Color::Yellow).bg(Color::Black));

    let mut items: Vec<ListItem> = Vec::new();
    for (i, (path, name)) in app.all_projects.iter().enumerate() {
        let prefix = if i == app.project_picker_idx { "▶ " } else { "  " };
        let path_short = path.to_string_lossy().to_string();
        let label = format!("{} {} ({})", prefix, name, path_short);
        let style = if i == app.project_picker_idx {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        items.push(ListItem::new(Span::styled(label, style)));
    }

    if items.is_empty() {
        items.push(ListItem::new(vec![Line::from(Span::styled(" No other projects found.", Style::default().fg(Color::Gray)))]));
    }

    let list = List::new(items);
    frame.render_widget(block, picker_area);
    frame.render_widget(list, picker_area);
}

/// Render the move column popup.
fn render_move_popup(frame: &mut Frame, _app: &App, area: Rect) {
    let popup_area = center_rect(area, 50, 8);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Move to... ")
        .style(Style::default().fg(Color::Cyan).bg(Color::Black));

    let columns = vec![
        ("b", "Backlog"),
        ("t", "Todo"),
        ("i", "In Progress"),
        ("r", "Review"),
        ("d", "Done"),
    ];

    let lines: Vec<Line> = columns.iter().map(|(key, name)| {
        Line::from(format!("  [{}] {}", key, name))
    }).collect();

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::White).bg(Color::Black));

    frame.render_widget(block, popup_area);
    frame.render_widget(paragraph, popup_area);
}

/// Render the search input overlay.
fn render_search_input(frame: &mut Frame, app: &App, area: Rect) {
    let popup_area = center_rect(area, 50, 3);

    let input = format!(" Search: {}█", app.search_query);
    let paragraph = Paragraph::new(input)
        .block(Block::default().borders(Borders::ALL).title(" Search "))
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::Yellow).bg(Color::Black));

    frame.render_widget(paragraph, popup_area);
}

/// Render a card's detail information as lines of text.
fn render_card_detail<'a>(card: &'a Card) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        &card.title,
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )));

    // Metadata
    let labels_str = if card.labels.is_empty() {
        "none".to_string()
    } else {
        card.labels.join(", ")
    };
    let meta = format!(
        "ID: {} | Priority: {} | Labels: {}",
        &card.id.chars().take(12).collect::<String>(),
        card.priority.to_string(),
        labels_str
    );
    lines.push(Line::from(Span::styled(meta, Style::default().fg(Color::Gray))));

    // Separator
    lines.push(Line::from("─".repeat(38)));

    // Description body (rendered as plain text)
    let body = &card.description;
    let mut remaining_lines = body.lines();
    for line in remaining_lines.by_ref() {
        if lines.len() >= 20 { break; }
        lines.push(Line::from(truncate(line, 36)));
    }

    // If description is truncated, show indicator
    if remaining_lines.next().is_some() {
        lines.push(Line::from(Span::styled("...(truncated)", Style::default().fg(Color::Gray))));
    }

    // Empty state
    if card.description.is_empty() {
        lines.push(Line::from(Span::styled(" (no description)", Style::default().fg(Color::Gray))));
    }

    lines
}

/// Priority indicator character.
fn priority_indicator(p: &Priority) -> Span<'static> {
    let (ch, color) = match p {
        Priority::Urgent => ("!", Color::Red),
        Priority::High => ("!", Color::Magenta),
        Priority::Medium => ("·", Color::Yellow),
        Priority::Low => (".", Color::Green),
        Priority::Backlog => (" ", Color::Gray),
    };
    Span::styled(ch.to_string(), Style::default().fg(color))
}

/// Truncate a string to fit within `max_width` characters.
fn truncate(s: &str, max_width: usize) -> String {
    if max_width <= 3 {
        return "…".to_string();
    }
    if s.chars().count() > max_width {
        s.chars().take(max_width - 1).collect::<String>() + "…"
    } else {
        s.to_string()
    }
}

/// Center a rectangle within another.
fn center_rect(r: Rect, width: u16, height: u16) -> Rect {
    let x = r.x + r.width.saturating_sub(width).saturating_div(2);
    let y = r.y + r.height.saturating_sub(height).saturating_div(2);
    Rect::new(
        x.min(r.x + r.width - width),
        y.min(r.y + r.height - height),
        width.min(r.width),
        height.min(r.height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;

    use crate::tui::app::ColumnView;

    fn test_app() -> App {
        App {
            running: true,
            focus: Focus::Cards,
            mode: Mode::Normal,
            error: None,
            project_path: PathBuf::from("/tmp/test-project"),
            board_name: "Test Board".to_string(),
            columns: vec![ColumnView {
                name: "todo".to_string(),
                cards: vec![],
            }],
            all_cards: vec![],
            current_column_idx: 0,
            card_selection: 0,
            detail_card: None,
            search_query: String::new(),
            search_results: vec![],
            all_projects: vec![],
            project_picker_idx: 0,
            message: None,
            message_time: std::time::Instant::now(),
        }
    }

    #[test]
    fn render_bottom_statusbar_displays_errors() {
        let backend = TestBackend::new(100, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();
        app.error = Some("Card not found: not-a-card".to_string());

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            rendered.contains("Card not found: not-a-card"),
            "rendered buffer did not contain error: {rendered}"
        );
    }

    #[test]
    fn render_bottom_statusbar_does_not_advertise_markdown_editing() {
        let backend = TestBackend::new(100, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = test_app();

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!rendered.contains("Edit"), "rendered buffer advertised editing: {rendered}");
        assert!(
            !rendered.contains("e Edit"),
            "rendered buffer advertised e key editing: {rendered}"
        );
    }
}
