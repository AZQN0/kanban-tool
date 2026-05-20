use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Priority {
    Backlog,
    Low,
    Medium,
    High,
    Urgent,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Backlog => write!(f, "backlog"),
            Priority::Low => write!(f, "low"),
            Priority::Medium => write!(f, "medium"),
            Priority::High => write!(f, "high"),
            Priority::Urgent => write!(f, "urgent"),
        }
    }
}

impl Priority {
    pub fn from_str(s: &str) -> Option<Priority> {
        match s {
            "backlog" => Some(Priority::Backlog),
            "low" => Some(Priority::Low),
            "medium" => Some(Priority::Medium),
            "high" => Some(Priority::High),
            "urgent" => Some(Priority::Urgent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub board_id: String,
    pub column_id: String,
    pub title: String,
    pub description: String,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub subtasks: Vec<String>,
    pub parent_card_id: Option<String>,
    pub card_file: PathBuf,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Card {
    pub fn new(
        board_id: &str,
        column_id: &str,
        title: &str,
        description: &str,
        priority: Priority,
        labels: Vec<String>,
        card_file: PathBuf,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            board_id: board_id.to_string(),
            column_id: column_id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            priority,
            labels,
            subtasks: vec![],
            parent_card_id: None,
            card_file,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update(
        &mut self,
        title: Option<&str>,
        description: Option<&str>,
        column_id: Option<&str>,
        priority: Option<Priority>,
        labels: Option<Vec<String>>,
    ) {
        if let Some(t) = title {
            self.title = t.to_string();
        }
        if let Some(d) = description {
            self.description = d.to_string();
        }
        if let Some(c) = column_id {
            self.column_id = c.to_string();
        }
        if let Some(p) = priority {
            self.priority = p;
        }
        if let Some(l) = labels {
            self.labels = l;
        }
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardRow {
    pub id: String,
    pub board_id: String,
    pub column_id: String,
    pub title: String,
    pub description: String,
    pub priority: String,
    pub labels: String,
    pub subtasks: String,
    pub parent_card_id: Option<String>,
    pub card_file: String,
    pub created_at: String,
    pub updated_at: String,
}

impl CardRow {
    pub fn to_card(&self) -> Card {
        Card {
            id: self.id.clone(),
            board_id: self.board_id.clone(),
            column_id: self.column_id.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            priority: Priority::from_str(&self.priority).unwrap_or(Priority::Backlog),
            labels: serde_json::from_str(&self.labels).unwrap_or_default(),
            subtasks: serde_json::from_str(&self.subtasks).unwrap_or_default(),
            parent_card_id: self.parent_card_id.clone(),
            card_file: PathBuf::from(&self.card_file),
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }
    }
}

impl Card {
    pub fn to_row(&self) -> CardRow {
        CardRow {
            id: self.id.clone(),
            board_id: self.board_id.clone(),
            column_id: self.column_id.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            priority: self.priority.to_string(),
            labels: serde_json::to_string(&self.labels).unwrap_or_default(),
            subtasks: serde_json::to_string(&self.subtasks).unwrap_or_default(),
            parent_card_id: self.parent_card_id.clone(),
            card_file: self.card_file.to_string_lossy().to_string(),
            created_at: self.created_at.to_rfc3339(),
            updated_at: self.updated_at.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_roundtrip() {
        for p in [
            Priority::Backlog,
            Priority::Low,
            Priority::Medium,
            Priority::High,
            Priority::Urgent,
        ] {
            let s = p.to_string();
            assert_eq!(Priority::from_str(&s), Some(p));
        }
    }

    #[test]
    fn test_card_new() {
        let card = Card::new(
            "board-1",
            "col-1",
            "Test",
            "",
            Priority::Medium,
            vec!["bug".to_string()],
            PathBuf::from("test.md"),
        );
        assert_eq!(card.title, "Test");
        assert_eq!(card.priority, Priority::Medium);
        assert_eq!(card.labels, vec!["bug"]);
    }

    #[test]
    fn test_card_update() {
        let mut card = Card::new(
            "b",
            "c",
            "Old",
            "",
            Priority::Low,
            vec![],
            PathBuf::from("x.md"),
        );
        card.update(Some("New"), None, None, Some(Priority::High), None);
        assert_eq!(card.title, "New");
        assert_eq!(card.priority, Priority::High);
    }

    #[test]
    fn test_card_row_roundtrip() {
        let card = Card::new(
            "b",
            "c",
            "Test",
            "desc",
            Priority::High,
            vec!["a".to_string(), "b".to_string()],
            PathBuf::from("test.md"),
        );
        let row = card.to_row();
        let back = row.to_card();
        assert_eq!(back.id, card.id);
        assert_eq!(back.title, card.title);
        assert_eq!(back.priority, card.priority);
        assert_eq!(back.labels, card.labels);
    }
}
