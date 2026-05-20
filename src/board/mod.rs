#![allow(dead_code)]

pub mod card;
pub mod column;
pub mod label;
pub mod store;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: String,
    pub project_path: String,
    pub name: String,
    pub columns: Vec<column::Column>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardRow {
    pub id: String,
    pub project_path: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

impl BoardRow {
    pub fn into_board(self, columns: Vec<column::Column>) -> Board {
        Board {
            id: self.id,
            project_path: self.project_path,
            name: self.name,
            columns,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub card_id: String,
    pub author: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl Comment {
    pub fn new(card_id: &str, author: &str, content: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            card_id: card_id.to_string(),
            author: author.to_string(),
            content: content.to_string(),
            created_at: Utc::now(),
        }
    }
}
