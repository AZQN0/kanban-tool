#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::DateTime;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::fs;
use std::path::Path;

use crate::board::card::{Card, Priority};
use crate::board::column::Column;

/// Parse a markdown card file into a Card struct.
pub fn parse_card_file(path: &Path) -> Result<Card> {
    let content =
        fs::read_to_string(path).context(format!("Failed to read card file: {:?}", path))?;
    parse_card_content(&content, path.to_path_buf())
}

/// Parse markdown content string into a Card.
pub fn parse_card_content(content: &str, card_file: std::path::PathBuf) -> Result<Card> {
    let mut frontmatter = CardFrontmatter::new();
    let mut in_frontmatter = false;
    let mut body_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        if !in_frontmatter && line.trim() == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter && line.trim() == "---" {
            in_frontmatter = false;
            continue;
        }
        if in_frontmatter {
            parse_frontmatter_field(line, &mut frontmatter);
        } else {
            body_lines.push(line.to_string());
        }
    }

    if frontmatter.id.is_empty() {
        anyhow::bail!("Card file missing frontmatter (---delimiters)");
    }

    let body = body_lines.join("\n");
    let description = render_markdown(&body);

    Ok(Card {
        id: frontmatter.id,
        board_id: frontmatter.board_id,
        column_id: frontmatter.column_id,
        title: frontmatter.title,
        description,
        priority: frontmatter.priority.unwrap_or(Priority::Backlog),
        labels: frontmatter.labels.unwrap_or_default(),
        subtasks: frontmatter.subtasks.unwrap_or_default(),
        parent_card_id: frontmatter.parent_card_id,
        card_file,
        created_at: frontmatter.created_at,
        updated_at: frontmatter.updated_at,
    })
}

struct CardFrontmatter {
    id: String,
    board_id: String,
    column_id: String,
    title: String,
    priority: Option<Priority>,
    labels: Option<Vec<String>>,
    subtasks: Option<Vec<String>>,
    parent_card_id: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl CardFrontmatter {
    fn new() -> Self {
        let now = chrono::Utc::now();
        Self {
            id: String::new(),
            board_id: String::new(),
            column_id: String::new(),
            title: String::new(),
            priority: None,
            labels: None,
            subtasks: None,
            parent_card_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}

fn parse_frontmatter_field(line: &str, frontmatter: &mut CardFrontmatter) {
    if let Some(rest) = line.strip_prefix("id: ") {
        frontmatter.id = parse_frontmatter_string(rest);
    } else if let Some(rest) = line.strip_prefix("board_id: ") {
        frontmatter.board_id = parse_frontmatter_string(rest);
    } else if let Some(rest) = line.strip_prefix("column_id: ") {
        frontmatter.column_id = parse_frontmatter_string(rest);
    } else if let Some(rest) = line.strip_prefix("title: ") {
        frontmatter.title = parse_frontmatter_string(rest);
    } else if let Some(rest) = line.strip_prefix("priority: ") {
        frontmatter.priority = Priority::from_str(rest.trim());
    } else if let Some(rest) = line.strip_prefix("labels: ") {
        let trimmed = rest.trim();
        if trimmed != "[]" {
            frontmatter.labels = serde_json::from_str(trimmed).ok();
        }
    } else if let Some(rest) = line.strip_prefix("subtasks: ") {
        let trimmed = rest.trim();
        if trimmed != "[]" {
            frontmatter.subtasks = serde_json::from_str(trimmed).ok();
        }
    } else if let Some(rest) = line.strip_prefix("parent_card_id: ") {
        let trimmed = rest.trim();
        if trimmed != "null" {
            frontmatter.parent_card_id = Some(parse_frontmatter_string(rest));
        }
    } else if let Some(rest) = line.strip_prefix("created_at: ") {
        if let Ok(dt) = DateTime::parse_from_rfc3339(rest.trim()) {
            frontmatter.created_at = dt.with_timezone(&chrono::Utc);
        }
    } else if let Some(rest) = line.strip_prefix("updated_at: ") {
        if let Ok(dt) = DateTime::parse_from_rfc3339(rest.trim()) {
            frontmatter.updated_at = dt.with_timezone(&chrono::Utc);
        }
    }
}

fn parse_frontmatter_string(value: &str) -> String {
    let trimmed = value.trim();
    serde_json::from_str::<String>(trimmed).unwrap_or_else(|_| trimmed.to_string())
}

/// Render markdown body text to plain text for storage.
pub fn render_markdown(md: &str) -> String {
    let options = Options::all();
    let parser = Parser::new_ext(md, options);

    let mut result = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                result.push_str("```\n");
            }
            Event::End(TagEnd::CodeBlock) => {
                result.push_str("```\n");
            }
            Event::Text(text) => {
                result.push_str(&text);
            }
            Event::SoftBreak => result.push(' '),
            Event::HardBreak => result.push('\n'),
            Event::Start(Tag::Heading { level: _, .. }) => result.push('\n'),
            Event::End(TagEnd::Heading(_)) => result.push('\n'),
            _ => {} // Skip other markdown syntax
        }
    }

    result.trim().to_string()
}

/// Parse all markdown card files in a directory.
pub fn parse_cards_in_dir(dir_path: &Path) -> Result<Vec<Card>> {
    if !dir_path.exists() {
        return Ok(Vec::new());
    }

    let mut cards = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(dir_path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .collect();

    // Sort by filename for consistent ordering
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if let Ok(card) = parse_card_file(&entry.path()) {
            cards.push(card);
        }
    }

    Ok(cards)
}

/// Parse a column definition from markdown.
pub fn parse_column_file(path: &Path) -> Result<Column> {
    let content =
        fs::read_to_string(path).context(format!("Failed to read column file: {:?}", path))?;

    let mut board_id = String::new();
    let mut name = String::new();
    let mut sort_order: u32 = 0;

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("id: ") {
            let _id = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("board_id: ") {
            board_id = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("name: ") {
            name = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("sort_order: ") {
            sort_order = rest.trim().parse().unwrap_or(0);
        }
    }

    Ok(Column::new(&board_id, &name, sort_order))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_card_file() {
        let dir = std::env::temp_dir().join("kanban_test");
        std::fs::create_dir_all(&dir).unwrap();

        let card_path = dir.join("test-card.md");
        let mut file = std::fs::File::create(&card_path).unwrap();
        use std::io::Write;
        writeln!(file, "---").unwrap();
        writeln!(file, "id: test-123").unwrap();
        writeln!(file, "board_id: board-1").unwrap();
        writeln!(file, "column_id: col-todo").unwrap();
        writeln!(file, "title: Test Card").unwrap();
        writeln!(file, "priority: high").unwrap();
        writeln!(file, "labels: [\"bug\", \"frontend\"]").unwrap();
        writeln!(file, "---").unwrap();
        writeln!(file, "This is the card body.").unwrap();

        let card = parse_card_file(&card_path).unwrap();
        assert_eq!(card.id, "test-123");
        assert_eq!(card.title, "Test Card");
        assert_eq!(card.priority, Priority::High);
        assert_eq!(card.labels, vec!["bug".to_string(), "frontend".to_string()]);
        assert_eq!(card.description, "This is the card body.");

        // Cleanup
        std::fs::remove_file(&card_path).ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_parse_card_content() {
        let content = r#"---
id: card-1
board_id: board-1
column_id: col-done
title: Done Task
priority: medium
---
Some content here."#;

        let card = parse_card_content(content, std::path::PathBuf::from("test.md")).unwrap();
        assert_eq!(card.id, "card-1");
        assert_eq!(card.title, "Done Task");
        assert_eq!(card.priority, Priority::Medium);
        assert_eq!(card.description, "Some content here.");
    }

    #[test]
    fn parse_card_content_accepts_quoted_frontmatter_scalars() {
        let content = r#"---
id: "card-1"
board_id: "board-1"
column_id: "col-done"
title: "Title with \"quotes\"\nand colon: value"
priority: medium
labels: ["audit"]
parent_card_id: "parent:1"
---
Some content here."#;

        let card = parse_card_content(content, std::path::PathBuf::from("test.md")).unwrap();

        assert_eq!(card.id, "card-1");
        assert_eq!(card.title, "Title with \"quotes\"\nand colon: value");
        assert_eq!(card.parent_card_id.as_deref(), Some("parent:1"));
        assert_eq!(card.labels, vec!["audit"]);
    }

    #[test]
    fn test_render_markdown() {
        let md = "Hello world and code";
        let result = render_markdown(md);
        assert_eq!(result, "Hello world and code");
    }
}
