use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};
use std::path::Path;

use crate::board::card::{Card, Priority};
use crate::board::store::{Store, StoreError};
use crate::markdown::writer::{remove_card_file, sync_card};

#[derive(Debug, Clone, Default)]
pub struct CardPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub column_id: Option<String>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
}

pub fn create_card_with_markdown(
    store: &mut Store,
    card: &Card,
    cards_dir: &Path,
) -> Result<String> {
    let tx = store
        .conn
        .transaction()
        .context("Failed to begin card create transaction")?;

    insert_card(&tx, card)?;
    sync_card(card, cards_dir).context("Failed to write card markdown export")?;

    tx.commit()
        .context("Failed to commit card create transaction")?;
    Ok(card.id.clone())
}

pub fn update_card_with_markdown(
    store: &mut Store,
    card_id: &str,
    patch: CardPatch,
    cards_dir: &Path,
) -> Result<Card> {
    let tx = store
        .conn
        .transaction()
        .context("Failed to begin card update transaction")?;

    let mut card = get_card(&tx, card_id)?;
    if let Some(column_id) = patch.column_id.as_deref() {
        validate_column_for_card_board(&tx, &card.board_id, column_id)?;
    }

    if patch.has_changes() {
        card.update(
            patch.title.as_deref(),
            patch.description.as_deref(),
            patch.column_id.as_deref(),
            patch.priority,
            patch.labels,
        );
        update_card(&tx, &mut card)?;
    }

    sync_card(&card, cards_dir).context("Failed to update card markdown export")?;

    tx.commit()
        .context("Failed to commit card update transaction")?;
    Ok(card)
}

pub fn move_card_with_markdown(
    store: &mut Store,
    card_id: &str,
    column_id: &str,
    cards_dir: &Path,
) -> Result<Card> {
    update_card_with_markdown(
        store,
        card_id,
        CardPatch {
            column_id: Some(column_id.to_string()),
            ..CardPatch::default()
        },
        cards_dir,
    )
}

pub fn delete_card_with_markdown(
    store: &mut Store,
    card_id: &str,
    cards_dir: &Path,
) -> Result<Card> {
    let tx = store
        .conn
        .transaction()
        .context("Failed to begin card delete transaction")?;

    let card = get_card(&tx, card_id)?;
    let affected = tx
        .execute("DELETE FROM cards WHERE id = ?1", params![card_id])
        .context("Failed to delete card")?;
    if affected == 0 {
        return Err(StoreError::not_found("Card", card_id).into());
    }

    remove_card_file(card_id, cards_dir).context("Failed to remove card markdown export")?;

    tx.commit()
        .context("Failed to commit card delete transaction")?;
    Ok(card)
}

impl CardPatch {
    fn has_changes(&self) -> bool {
        self.title.is_some()
            || self.description.is_some()
            || self.column_id.is_some()
            || self.priority.is_some()
            || self.labels.is_some()
    }
}

fn insert_card(tx: &Transaction<'_>, card: &Card) -> Result<()> {
    let row = card.to_row();
    tx.execute(
        "INSERT INTO cards (id, board_id, column_id, title, description, priority, labels, subtasks, parent_card_id, card_file, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            &card.id,
            &card.board_id,
            &card.column_id,
            &card.title,
            &card.description,
            &row.priority,
            &row.labels,
            &row.subtasks,
            &row.parent_card_id,
            &row.card_file,
            &row.created_at,
            &row.updated_at,
        ],
    ).context("Failed to create card")?;
    Ok(())
}

fn get_card(tx: &Transaction<'_>, card_id: &str) -> Result<Card> {
    tx.query_row(
        "SELECT id, board_id, column_id, title, description, priority, labels, subtasks, parent_card_id, card_file, created_at, updated_at FROM cards WHERE id = ?1",
        params![card_id],
        Store::card_row_to_card,
    ).optional()
        .context("Failed to query card")?
        .ok_or_else(|| StoreError::not_found("Card", card_id).into())
}

fn update_card(tx: &Transaction<'_>, card: &mut Card) -> Result<()> {
    card.updated_at = Utc::now();
    let row = card.to_row();
    let affected = tx.execute(
        "UPDATE cards SET column_id = ?1, title = ?2, description = ?3, priority = ?4, labels = ?5, updated_at = ?6 WHERE id = ?7",
        params![
            &card.column_id,
            &card.title,
            &card.description,
            &row.priority,
            &row.labels,
            &row.updated_at,
            &card.id,
        ],
    ).context("Failed to update card")?;
    if affected == 0 {
        return Err(StoreError::not_found("Card", &card.id).into());
    }
    Ok(())
}

fn validate_column_for_card_board(
    tx: &Transaction<'_>,
    board_id: &str,
    column_id: &str,
) -> Result<()> {
    let column_board_id: String = tx
        .query_row(
            "SELECT board_id FROM columns WHERE id = ?1",
            params![column_id],
            |r| r.get(0),
        )
        .optional()
        .context("Failed to validate target column")?
        .ok_or_else(|| StoreError::BadInput(format!("Column not found: {}", column_id)))?;

    if column_board_id != board_id {
        return Err(StoreError::BadInput(format!(
            "Column {} does not belong to card board {}",
            column_id, board_id
        ))
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::board::card::{Card, Priority};
    use crate::board::column::Column;
    use crate::board::store::{Store, StoreError};
    use crate::persistence::{
        create_card_with_markdown, delete_card_with_markdown, update_card_with_markdown, CardPatch,
    };

    struct Fixture {
        _root: PathBuf,
        cards_dir: PathBuf,
        store: Store,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("kanban_persistence_test_{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            let db = root.join("kanban.db");
            let mut store = Store::open(&db).unwrap();
            store
                .create_board("board-1", root.to_string_lossy().as_ref(), "Test")
                .unwrap();
            store
                .add_column(&Column {
                    id: "todo".to_string(),
                    board_id: "board-1".to_string(),
                    name: "todo".to_string(),
                    sort_order: 0,
                })
                .unwrap();
            store
                .add_column(&Column {
                    id: "done".to_string(),
                    board_id: "board-1".to_string(),
                    name: "done".to_string(),
                    sort_order: 1,
                })
                .unwrap();

            let cards_dir = root.join("cards");
            Self {
                _root: root,
                cards_dir,
                store,
            }
        }

        fn card(&self, id: &str) -> Card {
            let mut card = Card::new(
                "board-1",
                "todo",
                "Original title",
                "Original description",
                Priority::Medium,
                vec!["initial".to_string()],
                PathBuf::from(format!("{id}.md")),
            );
            card.id = id.to_string();
            card
        }
    }

    fn read_export(cards_dir: &Path, id: &str) -> String {
        fs::read_to_string(cards_dir.join(format!("{id}.md"))).unwrap()
    }

    #[test]
    fn create_card_with_markdown_creates_row_and_export() {
        let mut fixture = Fixture::new();
        let card = fixture.card("card-create");

        let card_id =
            create_card_with_markdown(&mut fixture.store, &card, &fixture.cards_dir).unwrap();

        assert_eq!(card_id, "card-create");
        assert_eq!(
            fixture.store.get_card("card-create").unwrap().title,
            "Original title"
        );
        let export = read_export(&fixture.cards_dir, "card-create");
        assert!(export.contains("id: card-create"));
        assert!(export.contains("title: Original title"));
    }

    #[test]
    fn update_card_with_markdown_updates_row_and_export() {
        let mut fixture = Fixture::new();
        let card = fixture.card("card-update");
        create_card_with_markdown(&mut fixture.store, &card, &fixture.cards_dir).unwrap();

        let updated = update_card_with_markdown(
            &mut fixture.store,
            "card-update",
            CardPatch {
                title: Some("Updated title".to_string()),
                description: Some("Updated description".to_string()),
                column_id: Some("done".to_string()),
                priority: Some(Priority::High),
                labels: Some(vec!["audit".to_string(), "fix".to_string()]),
            },
            &fixture.cards_dir,
        )
        .unwrap();

        assert_eq!(updated.title, "Updated title");
        assert_eq!(updated.column_id, "done");
        assert_eq!(
            fixture.store.get_card("card-update").unwrap().priority,
            Priority::High
        );
        let export = read_export(&fixture.cards_dir, "card-update");
        assert!(export.contains("column_id: done"));
        assert!(export.contains("title: Updated title"));
        assert!(export.contains("Updated description"));
    }

    #[test]
    fn update_card_with_markdown_rolls_back_row_when_export_fails() {
        let mut fixture = Fixture::new();
        let card = fixture.card("card-update-failure");
        create_card_with_markdown(&mut fixture.store, &card, &fixture.cards_dir).unwrap();
        let blocking_path = fixture.cards_dir.join("not-a-directory");
        fs::write(&blocking_path, "not a directory").unwrap();

        let err = update_card_with_markdown(
            &mut fixture.store,
            "card-update-failure",
            CardPatch {
                title: Some("Should not persist".to_string()),
                description: None,
                column_id: None,
                priority: None,
                labels: None,
            },
            &blocking_path,
        )
        .unwrap_err();

        assert!(err.to_string().contains("markdown"));
        assert_eq!(
            fixture.store.get_card("card-update-failure").unwrap().title,
            "Original title"
        );
    }

    #[test]
    fn delete_card_with_markdown_removes_row_and_export() {
        let mut fixture = Fixture::new();
        let card = fixture.card("card-delete");
        create_card_with_markdown(&mut fixture.store, &card, &fixture.cards_dir).unwrap();

        let deleted =
            delete_card_with_markdown(&mut fixture.store, "card-delete", &fixture.cards_dir)
                .unwrap();

        assert_eq!(deleted.id, "card-delete");
        let err = fixture.store.get_card("card-delete").unwrap_err();
        assert!(matches!(
            err.downcast_ref::<StoreError>(),
            Some(StoreError::NotFound { .. })
        ));
        assert!(!fixture.cards_dir.join("card-delete.md").exists());
    }

    #[test]
    fn delete_card_with_markdown_keeps_row_when_export_removal_fails() {
        let mut fixture = Fixture::new();
        let card = fixture.card("card-delete-failure");
        fixture.store.create_card(&card).unwrap();
        fs::create_dir_all(fixture.cards_dir.join("card-delete-failure.md")).unwrap();

        let err = delete_card_with_markdown(
            &mut fixture.store,
            "card-delete-failure",
            &fixture.cards_dir,
        )
        .unwrap_err();

        assert!(err.to_string().contains("markdown"));
        assert_eq!(
            fixture.store.get_card("card-delete-failure").unwrap().title,
            "Original title"
        );
    }
}
