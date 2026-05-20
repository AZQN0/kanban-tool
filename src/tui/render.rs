use crate::board::card::{Card, Priority};

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::app::{App, EditorField, Focus, Mode};

/// Render the full TUI.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top status bar
            Constraint::Min(1),    // Main area
            Constraint::Length(3), // Bottom status bar
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
        Mode::Searching => format!(
            " 🔍 Searching: {} | Project: {} | Cards: {total}",
            app.search_query, project_name
        ),
        _ => format!(" 📋 Project: {} | Cards: {total}", project_name),
    };

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}

/// Main area with 3 panels: columns, cards, detail.
fn render_main_area(frame: &mut Frame, app: &App, area: Rect) {
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
            Constraint::Length(16), // Columns panel
            Constraint::Min(20),    // Cards panel
            Constraint::Length(40), // Detail panel
        ])
        .split(area);

    render_columns(frame, app, chunks[0]);
    render_cards(frame, app, chunks[1]);
    render_detail(frame, app, chunks[2]);

    if app.mode == Mode::Editing {
        render_editor(frame, app, area);
    }
}

/// Render the columns panel (left).
fn render_columns(frame: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = Vec::new();

    for col in &app.columns {
        let count = col.cards.len();
        items.push(ListItem::new(format!("{} ({})", col.name, count)));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Columns ")
                .style(Style::default().fg(Color::Gray))
                .border_style(panel_border_style(app.focus == Focus::Columns)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    if !app.columns.is_empty() {
        state.select(Some(app.current_column_idx));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

/// Render the cards panel (center).
fn render_cards(frame: &mut Frame, app: &App, area: Rect) {
    let cards = app.current_cards();

    // Get the current column name for the title
    let col_name = if app.mode == Mode::SearchingResult {
        "Search Results".to_string()
    } else {
        app.columns
            .get(app.current_column_idx)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "?".to_string())
    };

    let mut items: Vec<ListItem> = Vec::new();

    for card in cards {
        let priority_indicator = priority_indicator(&card.priority);
        let title = truncate(&card.title, area.width as usize - 5);

        let id_short: String = card.id.chars().take(8).collect();
        items.push(ListItem::new(format!(
            "{} {} {}",
            priority_indicator, id_short, title
        )));
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
                .border_style(panel_border_style(app.focus == Focus::Cards)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::REVERSED),
        );

    let mut state = ListState::default();
    if !cards.is_empty() {
        state.select(Some(app.card_selection));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

/// Render the card detail panel (right).
fn render_detail(frame: &mut Frame, app: &App, area: Rect) {
    let text = if let Some(ref card) = app.detail_card {
        render_card_detail(
            card,
            area.height.saturating_sub(2) as usize,
            area.width.saturating_sub(2) as usize,
        )
    } else {
        vec![Line::from(" No card selected. ")]
    };

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Detail ")
                .style(Style::default().fg(Color::Gray))
                .border_style(panel_border_style(app.focus == Focus::Detail)),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn panel_border_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    }
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
        format!(
            " {} | ↑↓ Navigate | Enter Focus | e Edit | m Move | D Delete | / Search | q Quit | {}",
            focus_indicator,
            app.message.as_ref().unwrap_or(&String::new())
        )
    } else {
        format!(
            " {} | ↑↓ Navigate | Enter Focus | e Edit | m Move | D Delete | / Search | q Quit",
            focus_indicator
        )
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

fn render_editor(frame: &mut Frame, app: &App, area: Rect) {
    let Some(editor) = &app.editor else {
        return;
    };
    if editor.editing_text {
        render_text_editor(frame, editor, area);
        return;
    }

    let popup_area = center_rect(area, 72, 14);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Edit Card ")
        .style(Style::default().fg(Color::Cyan).bg(Color::Black));
    let inner = block.inner(popup_area);

    let field_line = |field: EditorField, label: &str, value: String| {
        let marker = if editor.field == field { ">" } else { " " };
        Line::from(format!("{marker} {label:<12} {value}"))
    };

    let description =
        editor_display_value(&editor.description, editor.description_cursor, false, 52);
    let labels = editor_display_value(&editor.labels_input, editor.labels_cursor, false, 52);
    let title = editor_display_value(&editor.title, editor.title_cursor, false, 52);
    let priority = editor.priority.to_string();

    let lines = vec![
        field_line(EditorField::Title, "Title", title),
        field_line(EditorField::Description, "Description", description),
        field_line(EditorField::Priority, "Priority", priority),
        field_line(EditorField::Labels, "Labels", labels),
    ];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(inner);

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::White).bg(Color::Black));
    let help = Paragraph::new(vec![
        Line::from(" ↑↓/Tab Field  Enter/e Edit field  ←/→ Priority"),
        Line::from(" Ctrl+S Save card  Esc Cancel card"),
    ])
    .alignment(Alignment::Left)
    .style(Style::default().fg(Color::DarkGray).bg(Color::Black));

    frame.render_widget(block, popup_area);
    frame.render_widget(paragraph, chunks[0]);
    frame.render_widget(help, chunks[1]);
}

fn render_text_editor(frame: &mut Frame, editor: &super::app::EditorState, area: Rect) {
    let (label, value, cursor, multiline) = match editor.field {
        EditorField::Title => ("Title", editor.title.as_str(), editor.title_cursor, false),
        EditorField::Description => (
            "Description",
            editor.description.as_str(),
            editor.description_cursor,
            true,
        ),
        EditorField::Labels => (
            "Labels",
            editor.labels_input.as_str(),
            editor.labels_cursor,
            false,
        ),
        EditorField::Priority => return,
    };

    let popup_area = center_rect(area, 72, if multiline { 18 } else { 8 });
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Edit {label} "))
        .style(Style::default().fg(Color::Cyan).bg(Color::Black));
    let inner = block.inner(popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let lines = text_editor_lines(value, cursor, multiline, 64, chunks[0].height as usize);
    let help = if multiline {
        "Arrows Move  Enter Newline  Ctrl+S Done  Esc Cancel"
    } else {
        "←/→ Move  Home/End Jump  Enter/Ctrl+S Done  Esc Cancel"
    };

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::White).bg(Color::Black));
    let help = Paragraph::new(help)
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::DarkGray).bg(Color::Black));

    frame.render_widget(block, popup_area);
    frame.render_widget(paragraph, chunks[0]);
    frame.render_widget(help, chunks[1]);
}

/// Render the move column popup.
fn render_move_popup(frame: &mut Frame, _app: &App, area: Rect) {
    let popup_area = center_rect(area, 50, 8);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Move to... ")
        .style(Style::default().fg(Color::Cyan).bg(Color::Black));

    let columns = [
        ("b", "Backlog"),
        ("t", "Todo"),
        ("i", "In Progress"),
        ("r", "Review"),
        ("d", "Done"),
    ];

    let lines: Vec<Line> = columns
        .iter()
        .map(|(key, name)| Line::from(format!("  [{}] {}", key, name)))
        .collect();

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
fn render_card_detail<'a>(
    card: &'a Card,
    max_visible_lines: usize,
    max_line_width: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        &card.title,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));

    // Metadata
    let labels_str = if card.labels.is_empty() {
        "none".to_string()
    } else {
        card.labels.join(", ")
    };
    lines.push(Line::from(Span::styled(
        format!("ID: {}", &card.id.chars().take(12).collect::<String>()),
        Style::default().fg(Color::Gray),
    )));
    lines.push(Line::from(Span::styled(
        format!("Priority: {}", card.priority),
        Style::default().fg(Color::Gray),
    )));
    lines.push(Line::from(Span::styled(
        format!("Labels: {}", labels_str),
        Style::default().fg(Color::Gray),
    )));

    // Separator
    lines.push(Line::from("─".repeat(38)));

    // Empty state
    if card.description.is_empty() {
        if lines.len() < max_visible_lines {
            lines.push(Line::from(Span::styled(
                " (no description)",
                Style::default().fg(Color::Gray),
            )));
        }
        return lines;
    }

    let body_lines = wrap_text_lines(&card.description, max_line_width);
    let body_capacity = max_visible_lines.saturating_sub(lines.len());
    if body_lines.len() <= body_capacity {
        for line in body_lines {
            lines.push(Line::from(line));
        }
    } else if body_capacity > 0 {
        for line in body_lines.iter().take(body_capacity.saturating_sub(1)) {
            lines.push(Line::from(line.clone()));
        }
        lines.push(Line::from(Span::styled(
            "...(truncated)",
            Style::default().fg(Color::Gray),
        )));
    }

    lines
}

fn wrap_text_lines(text: &str, max_width: usize) -> Vec<String> {
    let width = max_width.max(1);
    let mut wrapped = Vec::new();

    for source_line in text.lines() {
        if source_line.is_empty() {
            wrapped.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in source_line.split_whitespace() {
            let current_len = current.chars().count();
            let word_len = word.chars().count();
            if current_len == 0 {
                push_wrapped_word(&mut wrapped, &mut current, word, width);
            } else if current_len + 1 + word_len <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                wrapped.push(std::mem::take(&mut current));
                push_wrapped_word(&mut wrapped, &mut current, word, width);
            }
        }
        if !current.is_empty() {
            wrapped.push(current);
        }
    }

    wrapped
}

fn push_wrapped_word(wrapped: &mut Vec<String>, current: &mut String, word: &str, width: usize) {
    let mut remaining = word;
    while remaining.chars().count() > width {
        let chunk = remaining.chars().take(width).collect::<String>();
        let consumed = chunk.len();
        wrapped.push(chunk);
        remaining = &remaining[consumed..];
    }
    current.push_str(remaining);
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

fn truncate_multiline(s: &str, max_width: usize) -> String {
    truncate(&s.replace('\n', " / "), max_width)
}

fn editor_display_value(value: &str, cursor: usize, active: bool, max_width: usize) -> String {
    if !active {
        return truncate_multiline(value, max_width);
    }

    let mut rendered = String::new();
    let mut inserted_cursor = false;
    for (idx, ch) in value.chars().enumerate() {
        if idx == cursor {
            rendered.push('█');
            inserted_cursor = true;
        }
        if ch == '\n' {
            rendered.push_str(" / ");
        } else {
            rendered.push(ch);
        }
    }
    if !inserted_cursor {
        rendered.push('█');
    }
    truncate_around_cursor(&rendered, max_width)
}

fn text_editor_lines(
    value: &str,
    cursor: usize,
    multiline: bool,
    max_width: usize,
    max_height: usize,
) -> Vec<Line<'static>> {
    if !multiline {
        return vec![Line::from(editor_display_value(
            value, cursor, true, max_width,
        ))];
    }

    let (lines, cursor_line) = multiline_editor_lines(value, cursor, max_width);
    if lines.len() <= max_height {
        return lines.into_iter().map(Line::from).collect();
    }

    let start = if cursor_line >= max_height {
        cursor_line + 1 - max_height
    } else {
        0
    };
    lines
        .into_iter()
        .skip(start)
        .take(max_height)
        .map(Line::from)
        .collect()
}

fn multiline_editor_lines(value: &str, cursor: usize, max_width: usize) -> (Vec<String>, usize) {
    let width = max_width.max(1);
    let mut marked = String::new();
    let mut inserted_cursor = false;
    for (idx, ch) in value.chars().enumerate() {
        if idx == cursor {
            marked.push('█');
            inserted_cursor = true;
        }
        marked.push(ch);
    }
    if !inserted_cursor {
        marked.push('█');
    }

    let mut rows = Vec::new();
    let mut cursor_line = 0;
    for logical_line in marked.split('\n') {
        let wrapped = wrap_editor_logical_line(logical_line, width);
        for row in wrapped {
            if row.contains('█') {
                cursor_line = rows.len();
            }
            rows.push(row);
        }
    }
    if rows.is_empty() {
        rows.push("█".to_string());
    }
    (rows, cursor_line)
}

fn wrap_editor_logical_line(line: &str, width: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }

    let mut rows = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let current_len = current.chars().count();
        let word_len = word.chars().count();
        if current_len == 0 {
            push_editor_word(&mut rows, &mut current, word, width);
        } else if current_len + 1 + word_len <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            rows.push(std::mem::take(&mut current));
            push_editor_word(&mut rows, &mut current, word, width);
        }
    }

    if !current.is_empty() {
        rows.push(current);
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

fn push_editor_word(rows: &mut Vec<String>, current: &mut String, word: &str, width: usize) {
    let mut remaining = word;
    while remaining.chars().count() > width {
        let chunk = remaining.chars().take(width).collect::<String>();
        let consumed = chunk.len();
        rows.push(chunk);
        remaining = &remaining[consumed..];
    }
    current.push_str(remaining);
}

fn truncate_around_cursor(value: &str, max_width: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= max_width {
        return value.to_string();
    }
    if max_width <= 3 {
        return "…".to_string();
    }

    let cursor_idx = chars.iter().position(|ch| *ch == '█').unwrap_or(0);
    let window_width = max_width - 1;
    let start = if cursor_idx >= window_width {
        cursor_idx + 1 - window_width
    } else {
        0
    };
    let end = (start + window_width).min(chars.len());
    let mut rendered = String::new();
    if start > 0 {
        rendered.push('…');
    }
    rendered.extend(chars[start..end].iter());
    rendered
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
            editor: None,
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
    fn render_bottom_statusbar_advertises_card_editor_not_markdown_editing() {
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
        assert!(
            rendered.contains("e Edit"),
            "rendered buffer did not advertise card editing: {rendered}"
        );
        assert!(
            !rendered.contains("P Project"),
            "rendered buffer advertised unsupported project switching: {rendered}"
        );
    }

    #[test]
    fn render_main_panel_border_reflects_focus() {
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();

        app.focus = Focus::Columns;
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 1)].fg, Color::Cyan);
        assert_eq!(terminal.backend().buffer()[(16, 1)].fg, Color::Gray);
        assert_eq!(terminal.backend().buffer()[(60, 1)].fg, Color::Gray);

        app.focus = Focus::Cards;
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 1)].fg, Color::Gray);
        assert_eq!(terminal.backend().buffer()[(16, 1)].fg, Color::Cyan);
        assert_eq!(terminal.backend().buffer()[(60, 1)].fg, Color::Gray);

        app.focus = Focus::Detail;
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 1)].fg, Color::Gray);
        assert_eq!(terminal.backend().buffer()[(16, 1)].fg, Color::Gray);
        assert_eq!(terminal.backend().buffer()[(60, 1)].fg, Color::Cyan);
    }

    #[test]
    fn render_lists_use_highlight_without_cursor_prefixes() {
        let backend = TestBackend::new(100, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();
        let mut card = Card::new(
            "board-1",
            "todo",
            "Database index/perf migration",
            "Description",
            Priority::High,
            vec![],
            PathBuf::from("card.md"),
        );
        card.id = "80cb3f7f-card".to_string();
        app.columns[0].cards = vec![card.clone()];
        app.all_cards = vec![card];

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            !rendered.contains('▶'),
            "rendered buffer used a cursor prefix instead of highlight: {rendered}"
        );
    }

    #[test]
    fn render_card_detail_splits_metadata_across_lines() {
        let mut card = Card::new(
            "board-1",
            "todo",
            "Database index/perf migration",
            "Description",
            Priority::High,
            vec!["backend".to_string(), "reliability".to_string()],
            PathBuf::from("card.md"),
        );
        card.id = "1eb51d69-a3c-extra".to_string();

        let rendered = render_card_detail(&card, 20, 38)
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(rendered.iter().any(|line| line == "ID: 1eb51d69-a3c"));
        assert!(rendered.iter().any(|line| line == "Priority: high"));
        assert!(rendered
            .iter()
            .any(|line| line == "Labels: backend, reliability"));
        assert!(
            rendered.iter().all(|line| !line.contains(" | ")),
            "metadata should not be combined with pipe separators: {rendered:?}"
        );
    }

    #[test]
    fn render_card_detail_uses_available_height_before_truncating_description() {
        let mut card = Card::new(
            "board-1",
            "todo",
            "Tall detail",
            &(1..=25)
                .map(|line| format!("description line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
            Priority::Medium,
            vec![],
            PathBuf::from("card.md"),
        );
        card.id = "detail-height-card".to_string();

        let rendered = render_card_detail(&card, 32, 38)
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(
            rendered.iter().any(|line| line == "description line 25"),
            "detail should use available vertical space before truncating: {rendered:?}"
        );
        assert!(
            rendered.iter().all(|line| line != "...(truncated)"),
            "detail should not truncate when all lines fit: {rendered:?}"
        );
    }

    #[test]
    fn render_card_detail_wraps_long_description_lines_before_truncating() {
        let mut card = Card::new(
            "board-1",
            "todo",
            "Wrapped detail",
            "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda",
            Priority::Medium,
            vec![],
            PathBuf::from("card.md"),
        );
        card.id = "detail-wrap-card".to_string();

        let rendered = render_card_detail(&card, 12, 20)
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(
            rendered.iter().any(|line| line == "lambda"),
            "detail should wrap long description lines into available rows: {rendered:?}"
        );
        assert!(
            rendered.iter().all(|line| !line.contains('…')),
            "detail should not horizontally ellipsize wrapped description: {rendered:?}"
        );
    }

    #[test]
    fn render_editor_modal_displays_editable_fields_and_help() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();
        app.mode = Mode::Editing;
        app.editor = Some(super::super::app::EditorState {
            card_id: "card-1".to_string(),
            field: super::super::app::EditorField::Title,
            editing_text: false,
            text_edit_original: None,
            title: "Edit me".to_string(),
            title_cursor: "Edit me".chars().count(),
            description: "Description text".to_string(),
            description_cursor: "Description text".chars().count(),
            priority: Priority::High,
            labels_input: "ui, audit".to_string(),
            labels_cursor: "ui, audit".chars().count(),
            dirty: false,
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            rendered.contains("Edit Card"),
            "missing editor title: {rendered}"
        );
        assert!(
            rendered.contains("Title"),
            "missing title field: {rendered}"
        );
        assert!(
            rendered.contains("Description"),
            "missing description field: {rendered}"
        );
        assert!(
            rendered.contains("Priority"),
            "missing priority field: {rendered}"
        );
        assert!(
            rendered.contains("Labels"),
            "missing labels field: {rendered}"
        );
        assert!(
            rendered.contains("Ctrl+S Save"),
            "missing save hint: {rendered}"
        );
        assert!(
            rendered.contains("Esc Cancel"),
            "missing cancel hint: {rendered}"
        );
    }

    #[test]
    fn render_editor_modal_shows_cursor_in_active_text_field() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();
        app.mode = Mode::Editing;
        app.editor = Some(super::super::app::EditorState {
            card_id: "card-1".to_string(),
            field: super::super::app::EditorField::Title,
            editing_text: true,
            text_edit_original: Some("Edit me".to_string()),
            title: "Edit me".to_string(),
            title_cursor: 4,
            description: "Description text".to_string(),
            description_cursor: "Description text".chars().count(),
            priority: Priority::High,
            labels_input: "ui, audit".to_string(),
            labels_cursor: "ui, audit".chars().count(),
            dirty: false,
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            rendered.contains("Edit█ me"),
            "missing cursor at title insertion point: {rendered}"
        );
    }

    #[test]
    fn render_text_editor_keeps_cursor_visible_for_long_values() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();
        app.mode = Mode::Editing;
        app.editor = Some(super::super::app::EditorState {
            card_id: "card-1".to_string(),
            field: super::super::app::EditorField::Title,
            editing_text: true,
            text_edit_original: Some(
                "abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789"
                    .to_string(),
            ),
            title: "abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789"
                .to_string(),
            title_cursor: 70,
            description: String::new(),
            description_cursor: 0,
            priority: Priority::High,
            labels_input: String::new(),
            labels_cursor: 0,
            dirty: false,
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            rendered.contains('█'),
            "cursor should stay visible even when text is long: {rendered}"
        );
    }

    #[test]
    fn render_text_editor_pins_controls_to_popup_bottom() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();
        app.mode = Mode::Editing;
        app.editor = Some(super::super::app::EditorState {
            card_id: "card-1".to_string(),
            field: super::super::app::EditorField::Description,
            editing_text: true,
            text_edit_original: Some("line".to_string()),
            title: String::new(),
            title_cursor: 0,
            description: "line".to_string(),
            description_cursor: 4,
            priority: Priority::High,
            labels_input: String::new(),
            labels_cursor: 0,
            dirty: false,
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let help_y = buffer
            .content()
            .chunks(buffer.area.width as usize)
            .position(|row| {
                row.iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>()
                    .contains("Ctrl+S Done")
            })
            .expect("missing bottom controls");
        assert!(
            help_y >= 21,
            "controls should be pinned near popup bottom, found row {help_y}"
        );
    }

    #[test]
    fn render_multiline_text_editor_displays_real_lines_without_slash_separator() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();
        app.mode = Mode::Editing;
        app.editor = Some(super::super::app::EditorState {
            card_id: "card-1".to_string(),
            field: super::super::app::EditorField::Description,
            editing_text: true,
            text_edit_original: Some("first\nsecond".to_string()),
            title: String::new(),
            title_cursor: 0,
            description: "first\nsecond".to_string(),
            description_cursor: "first\nsecond".chars().count(),
            priority: Priority::High,
            labels_input: String::new(),
            labels_cursor: 0,
            dirty: false,
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("first"), "missing first line: {rendered}");
        assert!(
            rendered.contains("second█"),
            "missing second line with cursor: {rendered}"
        );
        assert!(
            !rendered.contains("first / second"),
            "multiline editor should not flatten lines with slash separators: {rendered}"
        );
    }

    #[test]
    fn render_multiline_text_editor_scrolls_to_keep_cursor_visible() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();
        let description = (1..=30)
            .map(|idx| format!("line {idx}"))
            .collect::<Vec<_>>()
            .join("\n");
        app.mode = Mode::Editing;
        app.editor = Some(super::super::app::EditorState {
            card_id: "card-1".to_string(),
            field: super::super::app::EditorField::Description,
            editing_text: true,
            text_edit_original: Some(description.clone()),
            title: String::new(),
            title_cursor: 0,
            description_cursor: description.chars().count(),
            description,
            priority: Priority::High,
            labels_input: String::new(),
            labels_cursor: 0,
            dirty: false,
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            rendered.contains("line 30█"),
            "cursor line should be visible: {rendered}"
        );
        assert!(
            !rendered.contains("line 1 "),
            "top lines should scroll out when cursor is near bottom: {rendered}"
        );
    }

    #[test]
    fn render_multiline_text_editor_wraps_long_description_lines() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = test_app();
        let description = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda";
        app.mode = Mode::Editing;
        app.editor = Some(super::super::app::EditorState {
            card_id: "card-1".to_string(),
            field: super::super::app::EditorField::Description,
            editing_text: true,
            text_edit_original: Some(description.to_string()),
            title: String::new(),
            title_cursor: 0,
            description: description.to_string(),
            description_cursor: description.chars().count(),
            priority: Priority::High,
            labels_input: String::new(),
            labels_cursor: 0,
            dirty: false,
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            rendered.contains("lambda█"),
            "wrapped final word and cursor should be visible: {rendered}"
        );
        assert!(
            !rendered.contains('…'),
            "multiline editor should wrap instead of horizontally scrolling: {rendered}"
        );
    }

    #[test]
    fn multiline_text_editor_lines_wrap_long_logical_lines() {
        let rendered = text_editor_lines(
            "alpha beta gamma delta epsilon zeta",
            "alpha beta gamma delta epsilon zeta".chars().count(),
            true,
            18,
            10,
        )
        .into_iter()
        .map(|line| {
            line.spans
                .into_iter()
                .map(|span| span.content.into_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>();

        assert!(
            rendered.len() > 1,
            "long logical lines should wrap into multiple visual rows: {rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line.contains("zeta█")),
            "cursor should remain on wrapped final row: {rendered:?}"
        );
        assert!(
            rendered.iter().all(|line| !line.contains('…')),
            "multiline wrapping should not use horizontal ellipsis: {rendered:?}"
        );
    }
}
