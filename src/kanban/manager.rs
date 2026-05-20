#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

use crate::board::store::Store;
use crate::board::Board;
use crate::kanban::config::{db_path, is_initialized};

/// Manages board discovery and database access for kanban projects.
///
/// A `BoardManager` wraps a `Store` and provides high-level operations
/// for finding, opening, and listing kanban boards across projects.
pub struct BoardManager {
    db_path: PathBuf,
}

impl BoardManager {
    /// Create a new BoardManager pointing at a specific database file.
    ///
    /// This does not open the connection yet — that happens on the first
    /// store operation.
    pub fn new(db_path: PathBuf) -> Result<Self> {
        Ok(Self { db_path })
    }

    /// Get the underlying store, opening the connection if needed.
    fn get_store(&self) -> Result<Store> {
        Store::open(&self.db_path).context(format!("Failed to open database at {:?}", self.db_path))
    }

    /// Find a board for the given project path.
    ///
    /// Returns `Some(board)` if the project is initialized with a kanban board,
    /// `None` if there is no `.kanban/` directory or the project is not in the database.
    pub fn find_board(&self, project_path: &Path) -> Result<Option<Board>> {
        let project_path = canonicalize_or_identity(project_path)?;

        if !is_initialized(&project_path) {
            return Ok(None);
        }

        let db = db_path(&project_path);
        let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;

        match store.get_board(project_path.to_string_lossy().as_ref()) {
            Ok(board) => Ok(Some(board)),
            Err(_) => Ok(None),
        }
    }

    /// Open a board for the given project path.
    ///
    /// Returns the full `Board` struct including all columns, or an error if
    /// the project is not initialized.
    pub fn open_board(&self, project_path: &Path) -> Result<Board> {
        let board = self
            .find_board(project_path)?
            .ok_or_else(|| anyhow!("No kanban board found at {:?}", project_path))?;
        Ok(board)
    }

    /// List all initialized boards across all projects in the database.
    pub fn list_all_projects(&self) -> Result<Vec<Board>> {
        let store = self.get_store()?;
        store.list_boards()
    }

    /// Get the project path for a given board ID.
    pub fn get_project_path(&self, board_id: &str) -> Result<String> {
        let store = self.get_store()?;
        store
            .get_board_id_for_path(board_id)
            .map_err(|_| anyhow!("No project path found for board ID: {}", board_id))
    }

    /// Initialize a new board at the given project path.
    ///
    /// Convenience wrapper that delegates to `crate::kanban::init::init_board`.
    pub fn init_board(&self, project_path: &Path) -> Result<Board> {
        crate::kanban::init::init_board(project_path)
    }

    /// Resolve a project path to a canonical board manager.
    ///
    /// If the project path contains a `.kanban/` directory, this creates a
    /// `BoardManager` pointing at that database. Otherwise, it returns `None`.
    pub fn for_project(project_path: &Path) -> Result<Option<BoardManager>> {
        let project_path = canonicalize_or_identity(project_path)?;
        if !is_initialized(&project_path) {
            return Ok(None);
        }
        let db = db_path(&project_path);
        BoardManager::new(db).map(Some)
    }
}

/// Try to canonicalize a path; fall back to the original if canonicalization fails.
fn canonicalize_or_identity(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).map_err(|e| anyhow!("Failed to resolve path {:?}: {}", path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_project() -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("kanban_test_mgr_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        (dir.clone(), db_path(&dir))
    }

    #[test]
    fn test_find_board_returns_none_for_uninitialized_project() {
        let (proj, _db) = setup_project();
        let manager = BoardManager::new(_db.clone()).unwrap();

        let result = manager.find_board(&proj).unwrap();
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_find_board_returns_some_for_initialized_project() {
        let (proj, _db) = setup_project();
        let board = crate::kanban::init::init_board(&proj).unwrap();
        let board_id = board.id.clone();

        // Create manager pointing at the db
        let manager = BoardManager::new(_db.clone()).unwrap();
        let result = manager.find_board(&proj).unwrap();
        assert!(result.is_some());
        let found = result.unwrap();
        assert_eq!(found.id, board_id);
        assert_eq!(found.columns.len(), 5);

        let _ = fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_open_board_fails_for_uninitialized_project() {
        let (proj, _db) = setup_project();
        let manager = BoardManager::new(_db).unwrap();

        let result = manager.open_board(&proj);
        assert!(result.is_err());

        let _ = fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_list_all_projects() {
        // Create two projects
        let proj1 = std::env::temp_dir().join(format!("kanban_proj1_{}", uuid::Uuid::new_v4()));
        let proj2 = std::env::temp_dir().join(format!("kanban_proj2_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&proj1).unwrap();
        fs::create_dir_all(&proj2).unwrap();

        // Initialize both with the same DB (simulating multi-project)
        let db = proj1.join(".kanban/kanban.db");
        // Note: In a real scenario, each project would have its own DB.
        // For this test, we just verify the list returns initialized projects.
        let _board1 = crate::kanban::init::init_board(&proj1).unwrap();
        let _board2 = crate::kanban::init::init_board(&proj2).unwrap();

        // List from proj1's DB (only proj1 is registered)
        let manager = BoardManager::new(db).unwrap();
        let projects = manager.list_all_projects().unwrap();
        assert!(!projects.is_empty());

        let _ = fs::remove_dir_all(&proj1);
        let _ = fs::remove_dir_all(&proj2);
    }

    #[test]
    fn test_board_manager_new() {
        let db = std::env::temp_dir().join("test.db");
        let manager = BoardManager::new(db.clone()).unwrap();
        // Verify it was created without opening the DB
        let _ = fs::remove_file(&db);
        let _ = manager;
    }
}
