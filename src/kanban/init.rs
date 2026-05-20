use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;

use crate::board::store::Store;
use crate::board::{column::Column, Board};
use crate::kanban::config::{cards_dir, columns_dir, db_path, is_initialized};

/// Initialize a new kanban board for a project at the given path.
///
/// Steps:
/// 1. Check if already initialized (returns error if so)
/// 2. Create `.kanban/`, `.kanban/cards/`, `.kanban/columns/` directories
/// 3. Open SQLite at `.kanban/kanban.db`
/// 4. Create board row (name = project directory name)
/// 5. Create default columns (backlog, todo, in_progress, review, done)
/// 6. Return the Board struct
pub fn init_board(project_path: &Path) -> Result<Board> {
    let project_path =
        fs::canonicalize(project_path).context("Failed to canonicalize project path")?;

    // Check if already initialized
    if is_initialized(&project_path) {
        return Err(anyhow!(
            "Kanban board already initialized at {}",
            project_path.display()
        ));
    }

    // Create directory structure
    let cards = cards_dir(&project_path);
    let columns = columns_dir(&project_path);
    let db = db_path(&project_path);

    fs::create_dir_all(&cards).context("Failed to create .kanban/cards/ directory")?;
    fs::create_dir_all(&columns).context("Failed to create .kanban/columns/ directory")?;

    // Determine board name from the project directory name
    let name = project_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string());

    // Create board in database
    let mut store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;

    let board_id = uuid::Uuid::new_v4().to_string();
    store
        .create_board(&board_id, project_path.to_string_lossy().as_ref(), &name)
        .context("Failed to create board record")?;

    // Create default columns
    let default_cols = Column::default_columns(&board_id);
    for col in &default_cols {
        store
            .add_column(col)
            .context(format!("Failed to create column: {}", col.name))?;
    }

    // Load and return the full board
    store
        .get_board(project_path.to_string_lossy().as_ref())
        .context("Failed to load newly created board")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_project() -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("kanban_test_init_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        (dir.clone(), db_path(&dir))
    }

    #[test]
    fn test_init_board_creates_directory_structure() {
        let (proj, _db) = temp_project();
        let board = init_board(&proj).unwrap();

        assert!(proj.join(".kanban").exists());
        assert!(proj.join(".kanban/cards").exists());
        assert!(proj.join(".kanban/columns").exists());
        assert!(proj.join(".kanban/kanban.db").exists());

        // Cleanup
        let _ = fs::remove_dir_all(&proj);
        assert_eq!(board.name, proj.file_name().unwrap().to_string_lossy());
    }

    #[test]
    fn test_init_board_creates_default_columns() {
        let (proj, _db) = temp_project();
        let board = init_board(&proj).unwrap();

        assert_eq!(board.columns.len(), 5);
        let names: Vec<&str> = board.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["backlog", "todo", "in_progress", "review", "done"]
        );

        // Cleanup
        let _ = fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_init_board_fails_when_already_initialized() {
        let (proj, _db) = temp_project();
        init_board(&proj).unwrap();

        let result = init_board(&proj);
        assert!(result.is_err());

        // Cleanup
        let _ = fs::remove_dir_all(&proj);
    }
}
