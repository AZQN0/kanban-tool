use crate::board::column::Column;

pub const DEFAULT_COLUMNS: &[&str] = &["backlog", "todo", "in_progress", "review", "done"];
pub const KANBAN_DIR: &str = ".kanban";
pub const DATABASE_NAME: &str = "kanban.db";
pub const CARDS_DIR: &str = "cards";

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
