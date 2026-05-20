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
    let content = fs::read_to_string(path)
        .context(format!("Failed to read card file: {:?}", path))?;
    parse_card_content(&content, path.to_path_buf())
}

/// Parse markdown content string into a Card.
pub fn parse_card_content(content: &str, card_file: std::path::PathBuf) -> Result<Card> {
    let mut id = String::new();
    let mut board_id = String::new();
    let mut column_id = String::new();
    let mut title = String::new();
    let mut priority: Option<Priority> = None;
    let mut labels: Option<Vec<String>> = None;
    let mut subtasks: Option<Vec<String>> = None;
    let mut parent_card_id: Option<String> = None;
    let mut created_at = chrono::Utc::now();
    let mut updated_at = chrono::Utc::now();
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
            parse_frontmatter_field(line, &mut id, &mut board_id, &mut column_id, &mut title,
                                    &mut priority, &mut labels, &mut subtasks, &mut parent_card_id,
                                    &mut created_at, &mut updated_at);
        } else {
            body_lines.push(line.to_string());
        }
    }
    
    if id.is_empty() {
        anyhow::bail!("Card file missing frontmatter (---delimiters)");
    }
    
    let body = body_lines.join("\n");
    let description = render_markdown(&body);
    
    Ok(Card {
        id,
        board_id,
        column_id,
        title,
        description,
        priority: priority.unwrap_or(Priority::Backlog),
        labels: labels.unwrap_or_default(),
        subtasks: subtasks.unwrap_or_default(),
        parent_card_id,
        card_file,
        created_at,
        updated_at,
    })
}

fn parse_frontmatter_field(line: &str, id: &mut String, board_id: &mut String,
                           column_id: &mut String, title: &mut String,
                           priority: &mut Option<Priority>, labels: &mut Option<Vec<String>>,
                           subtasks: &mut Option<Vec<String>>, parent_card_id: &mut Option<String>,
                           created_at: &mut chrono::DateTime<chrono::Utc>,
                           updated_at: &mut chrono::DateTime<chrono::Utc>) {
    if let Some(rest) = line.strip_prefix("id: ") {
        *id = rest.trim().to_string();
    } else if let Some(rest) = line.strip_prefix("board_id: ") {
        *board_id = rest.trim().to_string();
    } else if let Some(rest) = line.strip_prefix("column_id: ") {
        *column_id = rest.trim().to_string();
    } else if let Some(rest) = line.strip_prefix("title: ") {
        *title = rest.trim().to_string();
    } else if let Some(rest) = line.strip_prefix("priority: ") {
        *priority = Priority::from_str(rest.trim());
    } else if let Some(rest) = line.strip_prefix("labels: ") {
        let trimmed = rest.trim();
        if trimmed != "[]" {
            *labels = serde_json::from_str(trimmed).ok();
        }
    } else if let Some(rest) = line.strip_prefix("subtasks: ") {
        let trimmed = rest.trim();
        if trimmed != "[]" {
            *subtasks = serde_json::from_str(trimmed).ok();
        }
    } else if let Some(rest) = line.strip_prefix("parent_card_id: ") {
        *parent_card_id = Some(rest.trim().to_string());
    } else if let Some(rest) = line.strip_prefix("created_at: ") {
        if let Ok(dt) = DateTime::parse_from_rfc3339(rest.trim()) {
            *created_at = dt.with_timezone(&chrono::Utc);
        }
    } else if let Some(rest) = line.strip_prefix("updated_at: ") {
        if let Ok(dt) = DateTime::parse_from_rfc3339(rest.trim()) {
            *updated_at = dt.with_timezone(&chrono::Utc);
        }
    }
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
            Event::HardBreak => result.push_str("\n"),
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
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "md")
        })
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
    let content = fs::read_to_string(path)
        .context(format!("Failed to read column file: {:?}", path))?;
    
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
    fn test_render_markdown() {
        let md = "Hello world and code";
        let result = render_markdown(md);
        assert_eq!(result, "Hello world and code");
    }
}
