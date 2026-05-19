use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::kanban::config::DEFAULT_COLUMNS;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub sort_order: u32,
}

impl Column {
    pub fn new(board_id: &str, name: &str, sort_order: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            board_id: board_id.to_string(),
            name: name.to_string(),
            sort_order,
        }
    }

    pub fn default_columns(board_id: &str) -> Vec<Self> {
        DEFAULT_COLUMNS.iter().enumerate().map(|(i, name)| {
            Self {
                id: uuid::Uuid::new_v4().to_string(),
                board_id: board_id.to_string(),
                name: name.to_string(),
                sort_order: i as u32,
            }
        }).collect()
    }
}

impl Column {
    pub fn name_to_id(&self, name: &str) -> Option<String> {
        if self.name == name {
            Some(self.id.clone())
        } else {
            None
        }
    }

    pub fn find_by_name<'a>(columns: &'a [Column], name: &str) -> Option<&'a Column> {
        columns.iter().find(|c| c.name == name)
    }

    pub fn find_by_name_mut<'a>(columns: &'a mut [Column], name: &str) -> Option<&'a mut Column> {
        columns.iter_mut().find(|c| c.name == name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnRow {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub sort_order: i32,
}

impl ColumnRow {
    pub fn to_column(&self) -> Column {
        Column {
            id: self.id.clone(),
            board_id: self.board_id.clone(),
            name: self.name.clone(),
            sort_order: self.sort_order as u32,
        }
    }
}

impl Column {
    pub fn to_row(&self) -> ColumnRow {
        ColumnRow {
            id: self.id.clone(),
            board_id: self.board_id.clone(),
            name: self.name.clone(),
            sort_order: self.sort_order as i32,
        }
    }
}
