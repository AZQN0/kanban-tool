use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::board::card::Card;
use crate::board::column::Column;

/// Write a Card to a markdown file in the cards directory.
///
/// Creates the directory structure if it doesn't exist.
/// File naming: `<card_id>.md`
pub fn write_card(card: &Card, cards_dir: &Path) -> Result<()> {
    // Ensure the cards directory exists
    fs::create_dir_all(cards_dir)
        .context(format!("Failed to create cards directory: {:?}", cards_dir))?;
    
    let file_path = cards_dir.join(format!("{}.md", card.id));
    let content = build_card_markdown(card);
    
    fs::write(&file_path, content)
        .context(format!("Failed to write card file: {:?}", file_path))?;
    
    Ok(())
}

/// Build markdown content string from a Card.
fn build_card_markdown(card: &Card) -> String {
    let mut content = String::from("---\n");
    
    // Write frontmatter fields
    content.push_str(&format!("id: {}\n", card.id));
    content.push_str(&format!("board_id: {}\n", card.board_id));
    content.push_str(&format!("column_id: {}\n", card.column_id));
    content.push_str(&format!("title: {}\n", card.title));
    content.push_str(&format!("priority: {}\n", card.priority));
    
    // Labels as JSON array
    let labels_json = serde_json::to_string(&card.labels).unwrap_or_default();
    content.push_str(&format!("labels: {}\n", labels_json));
    
    // Subtasks as JSON array
    let subtasks_json = serde_json::to_string(&card.subtasks).unwrap_or_default();
    content.push_str(&format!("subtasks: {}\n", subtasks_json));
    
    // Parent card ID
    if let Some(ref parent_id) = card.parent_card_id {
        content.push_str(&format!("parent_card_id: {}\n", parent_id));
    } else {
        content.push_str("parent_card_id: null\n");
    }
    
    // Timestamps
    content.push_str(&format!("created_at: {}\n", card.created_at.to_rfc3339()));
    content.push_str(&format!("updated_at: {}\n", card.updated_at.to_rfc3339()));
    
    content.push_str("---\n");
    content.push('\n');
    
    // Description body
    content.push_str(&card.description);
    
    content
}

/// Write a Column definition to a markdown file.
///
/// File naming: `<column_id>.md` in the columns subdirectory.
pub fn write_column(column: &Column, columns_dir: &Path) -> Result<()> {
    fs::create_dir_all(columns_dir)
        .context(format!("Failed to create columns directory: {:?}", columns_dir))?;
    
    let file_path = columns_dir.join(format!("{}.md", column.id));
    let content = format!(
        "id: {}\nboard_id: {}\nname: {}\nsort_order: {}\n",
        column.id, column.board_id, column.name, column.sort_order
    );
    
    fs::write(&file_path, content)
        .context(format!("Failed to write column file: {:?}", file_path))?;
    
    Ok(())
}

/// Remove a card markdown file.
pub fn remove_card_file(card_id: &str, cards_dir: &Path) -> Result<()> {
    let file_path = cards_dir.join(format!("{}.md", card_id));
    if file_path.exists() {
        fs::remove_file(&file_path)
            .context(format!("Failed to remove card file: {:?}", file_path))?;
    }
    Ok(())
}

/// Remove a column markdown file.
pub fn remove_column_file(column_id: &str, columns_dir: &Path) -> Result<()> {
    let file_path = columns_dir.join(format!("{}.md", column_id));
    if file_path.exists() {
        fs::remove_file(&file_path)
            .context(format!("Failed to remove column file: {:?}", file_path))?;
    }
    Ok(())
}

/// Sync a card from SQLite to markdown file.
///
/// This is the primary write path - creates or updates the markdown representation.
pub fn sync_card(card: &Card, cards_dir: &Path) -> Result<()> {
    write_card(card, cards_dir)
}

/// Get the card file path for a given card ID.
pub fn card_file_path(card_id: &str, cards_dir: &Path) -> PathBuf {
    cards_dir.join(format!("{}.md", card_id))
}

/// Get the column file path for a given column ID.
pub fn column_file_path(column_id: &str, columns_dir: &Path) -> PathBuf {
    columns_dir.join(format!("{}.md", column_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::card::Priority;
    use chrono::Utc;

    fn test_card() -> Card {
        Card {
            id: "test-card".to_string(),
            board_id: "board-1".to_string(),
            column_id: "col-1".to_string(),
            title: "Test Card".to_string(),
            description: "Test description".to_string(),
            priority: Priority::High,
            labels: vec!["bug".to_string(), "frontend".to_string()],
            subtasks: vec!["subtask-1".to_string()],
            parent_card_id: None,
            card_file: PathBuf::from("test-card.md"),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_write_card() {
        let dir = std::env::temp_dir().join("kanban_test_write");
        let cards_dir = dir.join("cards");
        let _ = fs::remove_dir_all(&dir);
        
        let card = test_card();
        write_card(&card, &cards_dir).unwrap();
        
        let file_path = cards_dir.join("test-card.md");
        assert!(file_path.exists());
        
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("id: test-card"));
        assert!(content.contains("title: Test Card"));
        assert!(content.contains("priority: high"));
        assert!(content.contains("Test description"));
        
        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_build_card_markdown() {
        let card = test_card();
        let markdown = build_card_markdown(&card);
        
        assert!(markdown.starts_with("---\n"));
        assert!(markdown.contains("id: test-card\n"));
        assert!(markdown.contains("title: Test Card\n"));
        assert!(markdown.contains("priority: high\n"));
        assert!(markdown.contains("Test description"));
    }

    #[test]
    fn test_remove_card_file() {
        let dir = std::env::temp_dir().join("kanban_test_remove");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        
        // Create a dummy file
        let file_path = dir.join("test-123.md");
        fs::write(&file_path, "content").unwrap();
        
        // Remove it
        remove_card_file("test-123", &dir).unwrap();
        assert!(!file_path.exists());
        
        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_card_file_path() {
        let cards_dir = PathBuf::from("/test/cards");
        let path = card_file_path("abc-123", &cards_dir);
        assert_eq!(path, PathBuf::from("/test/cards/abc-123.md"));
    }

    #[test]
    fn test_column_file_path() {
        let columns_dir = PathBuf::from("/test/columns");
        let path = column_file_path("col-1", &columns_dir);
        assert_eq!(path, PathBuf::from("/test/columns/col-1.md"));
    }
}
