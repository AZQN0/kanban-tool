use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::Migrations;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::card::{Card, Priority};
use super::column::Column;
use super::Comment;

#[derive(Debug)]
pub enum StoreError {
    NotFound { resource: &'static str, id: String },
    BadInput(String),
}

impl StoreError {
    fn not_found(resource: &'static str, id: &str) -> Self {
        Self::NotFound {
            resource,
            id: id.to_string(),
        }
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { resource, id } => write!(f, "{} not found: {}", resource, id),
            Self::BadInput(message) => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for StoreError {}

fn s(r: &rusqlite::Row<'_>, idx: usize) -> String {
    r.get::<_, String>(idx).unwrap_or_default()
}

fn o(r: &rusqlite::Row<'_>, idx: usize) -> Option<String> {
    r.get::<_, Option<String>>(idx).unwrap_or(None)
}

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

pub struct Store {
    pub conn: Connection,
}

impl Store {
    /// Open the database, retrying once on transient WAL lock errors.
    pub fn open(path: &Path) -> Result<Self> {
        match Self::open_inner(path) {
            Ok(store) => Ok(store),
            Err(e) => {
                // WAL lock is transient — retry once after a short delay
                std::thread::sleep(Duration::from_millis(200));
                Self::open_inner(path)
                    .map_err(|_| anyhow!("Database temporarily locked (WAL lock). Retry later. Original error: {}", e))
            }
        }
    }

    fn open_inner(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .context(format!("Failed to open database at {:?}", path))?;
        // Set WAL mode for better concurrency (if not already WAL)
        let _ = conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;");
        let mut store = Self { conn };
        store.apply_migrations()?;
        Ok(store)
    }

    /// Open the database with WAL mode retry for concurrent access scenarios.
    pub fn open_with_retry(path: &Path) -> Result<Self> {
        let mut last_err = None;
        for attempt in 1..=5 {
            match Self::open_inner(path) {
                Ok(store) => return Ok(store),
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 5 {
                        std::thread::sleep(Duration::from_millis(100 * attempt as u64));
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("Failed to open database after retries")))
    }

    fn apply_migrations(&mut self) -> Result<()> {
        let migrations = Migrations::new(vec![
            rusqlite_migration::M::up(include_str!("../../migrations/001_init.sql")),
        ]);
        migrations.to_latest(&mut self.conn)
            .context("Failed to apply migrations")?;
        Ok(())
    }

    pub fn create_board(&mut self, id: &str, project_path: &str, name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO boards (id, project_path, name) VALUES (?1, ?2, ?3)",
            params![id, project_path, name],
        ).context("Failed to create board")?;
        Ok(())
    }

    pub fn get_board(&self, project_path: &str) -> Result<super::Board> {
        let (id, name, created_at, updated_at) = self.conn.query_row(
            "SELECT id, name, created_at, updated_at FROM boards WHERE project_path = ?1",
            params![project_path],
            |r| Ok((s(r, 0), s(r, 1), s(r, 2), s(r, 3))),
        ).context(format!("Board not found for path: {}", project_path))?;
        
        let columns = self.get_columns(&id)?;
        Ok(super::Board {
            id,
            project_path: project_path.to_string(),
            name,
            columns,
            created_at: parse_dt(&created_at),
            updated_at: parse_dt(&updated_at),
        })
    }

    pub fn list_boards(&self) -> Result<Vec<super::Board>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_path, name, created_at, updated_at FROM boards ORDER BY name"
        ).context("Failed to prepare boards query")?;
        
        let cards_rows = stmt.query_map([], |r| {
            Ok((s(r, 0), s(r, 1), s(r, 2), s(r, 3), s(r, 4)))
        }).context("Failed to query boards")?;
        
        let mut boards = Vec::new();
        for card_result in cards_rows {
            let (id, project_path, name, created_at, updated_at) = card_result?;
            let columns = self.get_columns(&id)?;
            boards.push(super::Board {
                id, project_path, name, columns,
                created_at: parse_dt(&created_at),
                updated_at: parse_dt(&updated_at),
            });
        }
        Ok(boards)
    }

    pub fn add_column(&mut self, column: &Column) -> Result<()> {
        self.conn.execute(
            "INSERT INTO columns (id, board_id, name, sort_order) VALUES (?1, ?2, ?3, ?4)",
            params![&column.id, &column.board_id, &column.name, column.sort_order],
        ).context("Failed to add column")?;
        Ok(())
    }

    pub fn add_default_columns(&mut self, board_id: &str) -> Result<()> {
        for col in Column::default_columns(board_id) {
            self.add_column(&col)?;
        }
        Ok(())
    }

    pub fn get_columns(&self, board_id: &str) -> Result<Vec<Column>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, board_id, name, sort_order FROM columns WHERE board_id = ?1 ORDER BY sort_order"
        ).context("Failed to prepare columns query")?;
        
        let mut cols = Vec::new();
        let rows = stmt.query_map(params![board_id], |r| {
            let sort_order: i32 = r.get(3)?;
            Ok(Column {
                id: s(r, 0),
                board_id: s(r, 1),
                name: s(r, 2),
                sort_order: sort_order as u32,
            })
        }).context("Failed to query columns")?;
        for col in rows {
            cols.push(col?);
        }
        Ok(cols)
    }

    pub fn get_column_by_name(&self, board_id: &str, name: &str) -> Result<Option<Column>> {
        let result: Option<Column> = self.conn.query_row(
            "SELECT id, board_id, name, sort_order FROM columns WHERE board_id = ?1 AND name = ?2",
            params![board_id, name],
            |r| {
                let sort_order: i32 = r.get(3)?;
                Ok(Column {
                    id: s(r, 0), board_id: s(r, 1), name: s(r, 2), sort_order: sort_order as u32,
                })
            },
        ).optional()?;
        Ok(result)
    }

    pub fn create_card(&mut self, card: &Card) -> Result<String> {
        let row = card.to_row();
        self.conn.execute(
            "INSERT INTO cards (id, board_id, column_id, title, description, priority, labels, subtasks, parent_card_id, card_file, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &card.id, &card.board_id, &card.column_id, &card.title,
                &card.description, &row.priority, &row.labels, &row.subtasks,
                &row.parent_card_id, &row.card_file, &row.created_at, &row.updated_at
            ],
        ).context("Failed to create card")?;
        Ok(card.id.clone())
    }

    pub fn get_card(&self, card_id: &str) -> Result<Card> {
        let row = self.conn.query_row(
            "SELECT id, board_id, column_id, title, description, priority, labels, subtasks, parent_card_id, card_file, created_at, updated_at FROM cards WHERE id = ?1",
            params![card_id],
            |r| Ok((
                s(r, 0), s(r, 1), s(r, 2), s(r, 3), s(r, 4), s(r, 5), s(r, 6),
                s(r, 7), o(r, 8), s(r, 9), s(r, 10), s(r, 11)
            )),
        ).optional()
            .context("Failed to query card")?
            .ok_or_else(|| StoreError::not_found("Card", card_id))?;

        let (id, board_id, column_id, title, description, priority, labels, subtasks, parent_card_id, card_file, created_at, updated_at) = row;
        
        Ok(Card {
            id, board_id, column_id, title, description,
            priority: Priority::from_str(&priority).unwrap_or(Priority::Backlog),
            labels: serde_json::from_str(&labels).unwrap_or_default(),
            subtasks: serde_json::from_str(&subtasks).unwrap_or_default(),
            parent_card_id,
            card_file: PathBuf::from(card_file),
            created_at: parse_dt(&created_at),
            updated_at: parse_dt(&updated_at),
        })
    }

    pub fn update_card(&mut self, card_id: &str, title: Option<&str>, description: Option<&str>,
                       column_id: Option<&str>, priority: Option<&str>, labels: Option<&[String]>) -> Result<()> {
        let mut updates: Vec<String> = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        
        if let Some(t) = title {
            updates.push("title = ?".to_string());
            values.push(Box::new(t.to_string()));
        }
        if let Some(d) = description {
            updates.push("description = ?".to_string());
            values.push(Box::new(d.to_string()));
        }
        if let Some(c) = column_id {
            updates.push("column_id = ?".to_string());
            values.push(Box::new(c.to_string()));
        }
        if let Some(p) = priority {
            updates.push("priority = ?".to_string());
            values.push(Box::new(p.to_string()));
        }
        if let Some(l) = labels {
            updates.push("labels = ?".to_string());
            values.push(Box::new(serde_json::to_string(l).unwrap_or_default()));
        }
        
        if updates.is_empty() {
            self.get_card(card_id)?;
            return Ok(());
        }
        
        let query = format!("UPDATE cards SET {} WHERE id = ?", updates.join(", "));
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = values;
        params.push(Box::new(card_id.to_string()));
        let affected = self.conn.execute(&query, rusqlite::params_from_iter(params.iter().map(|v| v.as_ref())))
            .context("Failed to update card")?;
        if affected == 0 {
            return Err(StoreError::not_found("Card", card_id).into());
        }
        Ok(())
    }

    pub fn delete_card(&mut self, card_id: &str) -> Result<()> {
        let affected = self.conn.execute(
            "DELETE FROM cards WHERE id = ?1",
            params![card_id],
        ).context("Failed to delete card")?;
        if affected == 0 {
            return Err(StoreError::not_found("Card", card_id).into());
        }
        Ok(())
    }

    fn card_row_to_card(row: &rusqlite::Row<'_>) -> rusqlite::Result<Card> {
        Ok(Card {
            id: s(row, 0), board_id: s(row, 1), column_id: s(row, 2),
            title: s(row, 3), description: s(row, 4),
            priority: Priority::from_str(&s(row, 5)).unwrap_or(Priority::Backlog),
            labels: serde_json::from_str(&s(row, 6)).unwrap_or_default(),
            subtasks: serde_json::from_str(&s(row, 7)).unwrap_or_default(),
            parent_card_id: o(row, 8),
            card_file: PathBuf::from(s(row, 9)),
            created_at: parse_dt(&s(row, 10)),
            updated_at: parse_dt(&s(row, 11)),
        })
    }

    pub fn list_cards(&self, board_id: &str, column_id: Option<&str>, priority: Option<&str>,
                      labels: Option<&[String]>, sort_by: &str) -> Result<Vec<Card>> {
        let mut query = String::from("SELECT id, board_id, column_id, title, description, priority, labels, subtasks, parent_card_id, card_file, created_at, updated_at FROM cards WHERE board_id = ?1");
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(board_id.to_string())];
        let mut param_idx: u32 = 2;
        
        if let Some(c) = column_id {
            query.push_str(&format!(" AND column_id = ?{}", param_idx));
            values.push(Box::new(c.to_string()));
            param_idx += 1;
        }
        if let Some(p) = priority {
            query.push_str(&format!(" AND priority = ?{}", param_idx));
            values.push(Box::new(p.to_string()));
            param_idx += 1;
        }
        if let Some(label_list) = labels {
            if !label_list.is_empty() {
                // Use LIKE on the JSON text column to match label names.
                // Labels are stored as JSON arrays like ["backend","security"],
                // so we search for "\"<label>\"" patterns.
                for label in label_list {
                    query.push_str(&format!(" AND labels LIKE ?{}", param_idx));
                    values.push(Box::new(format!("%\"{}\"%", label)));
                    param_idx += 1;
                }
            }
        }
        
        match sort_by {
            "priority" => {
                query.push_str(" ORDER BY CASE priority WHEN 'urgent' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 WHEN 'low' THEN 4 WHEN 'backlog' THEN 5 END");
            }
            "created" | _ => {
                query.push_str(" ORDER BY created_at ASC");
            }
        }
        
        let mut stmt = self.conn.prepare(&query).context("Failed to prepare card query")?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values.iter().map(|v| v.as_ref())), Self::card_row_to_card)
            .context("Failed to query cards")?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn search_cards(&self, board_id: &str, query_str: &str) -> Result<Vec<Card>> {
        let pattern = format!("%{}%", query_str);
        let mut stmt = self.conn.prepare(
            "SELECT id, board_id, column_id, title, description, priority, labels, subtasks, parent_card_id, card_file, created_at, updated_at FROM cards WHERE board_id = ?1 AND (title LIKE ?2 OR description LIKE ?2) ORDER BY created_at DESC"
        ).context("Failed to prepare search query")?;
        let rows = stmt.query_map(params![board_id, &pattern], Self::card_row_to_card)
            .context("Failed to query search results")?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn transition_card(&mut self, card_id: &str, new_column_id: &str) -> Result<()> {
        if self.conn.query_row(
            "SELECT 1 FROM columns WHERE id = ?1",
            params![new_column_id],
            |_| Ok(()),
        ).optional().context("Failed to validate target column")?.is_none() {
            return Err(StoreError::BadInput(format!("Column not found: {}", new_column_id)).into());
        }

        let affected = self.conn.execute(
            "UPDATE cards SET column_id = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![new_column_id, card_id],
        ).context("Failed to transition card")?;
        if affected == 0 {
            return Err(StoreError::not_found("Card", card_id).into());
        }
        Ok(())
    }

    pub fn add_comment(&mut self, comment: &Comment) -> Result<()> {
        self.conn.execute(
            "INSERT INTO comments (id, card_id, author, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&comment.id, &comment.card_id, &comment.author, &comment.content, &comment.created_at.to_rfc3339()],
        ).context("Failed to add comment")?;
        Ok(())
    }

    pub fn get_comments(&self, card_id: &str) -> Result<Vec<Comment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, card_id, author, content, created_at FROM comments WHERE card_id = ?1 ORDER BY created_at ASC"
        ).context("Failed to prepare comments query")?;
        
        let rows = stmt.query_map(params![card_id], |r| {
            Ok(Comment {
                id: s(r, 0), card_id: s(r, 1),
                author: s(r, 2), content: s(r, 3),
                created_at: parse_dt(&s(r, 4)),
            })
        }).context("Failed to query comments")?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn find_cards_by_parent(&self, parent_id: &str) -> Result<Vec<Card>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, board_id, column_id, title, description, priority, labels, subtasks, parent_card_id, card_file, created_at, updated_at FROM cards WHERE parent_card_id = ?1"
        ).context("Failed to prepare subtasks query")?;
        let rows = stmt.query_map(params![parent_id], Self::card_row_to_card)
            .context("Failed to query subtasks")?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn update_subtasks(&mut self, card_id: &str, subtask_ids: &[String]) -> Result<()> {
        let subtasks_json = serde_json::to_string(subtask_ids).unwrap_or_default();
        self.conn.execute(
            "UPDATE cards SET subtasks = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![&subtasks_json, card_id],
        ).context("Failed to update subtasks")?;
        Ok(())
    }

    pub fn get_board_id_for_path(&self, project_path: &str) -> Result<String> {
        self.conn.query_row(
            "SELECT id FROM boards WHERE project_path = ?1",
            params![project_path],
            |r| r.get(0),
        ).context(format!("No board found for path: {}", project_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> Store {
        let conn = Connection::open_in_memory().unwrap();
        let mut store = Store { conn };
        store.apply_migrations().unwrap();
        store.create_board("board-1", "/tmp/test-project", "Test Project").unwrap();
        store.add_column(&Column {
            id: "todo".to_string(),
            board_id: "board-1".to_string(),
            name: "todo".to_string(),
            sort_order: 0,
        }).unwrap();
        store.add_column(&Column {
            id: "done".to_string(),
            board_id: "board-1".to_string(),
            name: "done".to_string(),
            sort_order: 1,
        }).unwrap();
        store
    }

    fn test_card(id: &str) -> Card {
        let mut card = Card::new(
            "board-1",
            "todo",
            "Test card",
            "Description",
            Priority::Medium,
            vec![],
            PathBuf::from(format!("{id}.md")),
        );
        card.id = id.to_string();
        card
    }

    #[test]
    fn delete_card_returns_error_when_no_row_is_deleted() {
        let mut store = test_store();

        let err = store.delete_card("not-a-card").unwrap_err();

        assert!(err.to_string().contains("not-a-card"));
    }

    #[test]
    fn update_card_returns_error_when_no_row_is_updated() {
        let mut store = test_store();

        let err = store
            .update_card("not-a-card", Some("New title"), None, None, None, None)
            .unwrap_err();

        assert!(err.to_string().contains("not-a-card"));
    }

    #[test]
    fn transition_card_returns_error_when_no_row_is_updated() {
        let mut store = test_store();

        let err = store.transition_card("not-a-card", "done").unwrap_err();

        assert!(err.to_string().contains("not-a-card"));
    }

    #[test]
    fn delete_card_deletes_existing_card() {
        let mut store = test_store();
        let card = test_card("card-1");
        store.create_card(&card).unwrap();

        store.delete_card("card-1").unwrap();

        assert!(store.get_card("card-1").is_err());
    }
}
