use crate::board::column::Column;
use std::path::{Path, PathBuf};

pub const DEFAULT_COLUMNS: &[&str] = &["backlog", "todo", "in_progress", "review", "done"];
pub const KANBAN_DIR: &str = ".kanban";
pub const DATABASE_NAME: &str = "kanban.db";
pub const CARDS_DIR: &str = "cards";
pub const COLUMNS_DIR: &str = "columns";

pub struct BoardConfig {
    pub columns: Vec<Column>,
}

impl BoardConfig {
    pub fn new(board_id: &str) -> Self {
        Self {
            columns: Column::default_columns(board_id),
        }
    }
}

/// Get the `.kanban/` directory path for a project.
pub fn kanban_dir(project_path: &Path) -> PathBuf {
    project_path.join(KANBAN_DIR)
}

/// Get the database path for a project.
pub fn db_path(project_path: &Path) -> PathBuf {
    kanban_dir(project_path).join(DATABASE_NAME)
}

/// Get the cards directory path for a project.
pub fn cards_dir(project_path: &Path) -> PathBuf {
    kanban_dir(project_path).join(CARDS_DIR)
}

/// Get the columns directory path for a project.
pub fn columns_dir(project_path: &Path) -> PathBuf {
    kanban_dir(project_path).join(COLUMNS_DIR)
}

/// Check if a project has an initialized kanban board.
pub fn is_initialized(project_path: &Path) -> bool {
    kanban_dir(project_path).exists()
}
