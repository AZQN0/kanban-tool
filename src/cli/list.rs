use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::board::card::Card;
use crate::board::store::{CardSort, Store};
use crate::kanban::config::{db_path, is_initialized};
use crate::ListArgs;

/// List cards, optionally filtered by column, priority, labels, or project.
///
/// Prints a compact table to stdout.
pub fn list(args: &ListArgs) -> Result<()> {
    let resolved_project_path = resolve_project_path(&args.project)?;
    let project_path = match resolved_project_path {
        Some(path) => {
            if !is_initialized(&path) {
                anyhow::bail!("No kanban board found at {:?}", path);
            }
            path
        }
        None => {
            // No project specified — list from current directory if initialized
            let cwd = std::env::current_dir()?;
            if !is_initialized(&cwd) {
                anyhow::bail!("No project specified and no kanban board in current directory");
            }
            cwd
        }
    };

    let db = db_path(&project_path);
    let store = Store::open(&db)?;
    let snapshot = store
        .load_board_snapshot(project_path.to_string_lossy().as_ref(), CardSort::Created)
        .context(format!("Failed to open board at {:?}", project_path))?;

    let board = &snapshot.board;
    let column_id = args.column.as_ref().and_then(|name| {
        board
            .columns
            .iter()
            .find(|c| c.name == *name)
            .map(|c| c.id.as_str())
    });

    let cards = filter_cards(
        snapshot.all_cards,
        column_id,
        args.priority.as_deref(),
        args.label.as_deref(),
    );

    // Build a lookup from column_id -> column name
    let col_names: std::collections::HashMap<&str, &str> = board
        .columns
        .iter()
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();

    print_table(&cards, &col_names);
    Ok(())
}

fn filter_cards(
    cards: Vec<Card>,
    column_id: Option<&str>,
    priority: Option<&str>,
    labels: Option<&[String]>,
) -> Vec<Card> {
    cards
        .into_iter()
        .filter(|card| column_id.is_none_or(|id| card.column_id == id))
        .filter(|card| priority.is_none_or(|p| card.priority.to_string() == p))
        .filter(|card| {
            labels.is_none_or(|label_list| {
                label_list
                    .iter()
                    .all(|label| card.labels.iter().any(|card_label| card_label == label))
            })
        })
        .collect()
}

/// Resolve the project path from the optional --project argument.
fn resolve_project_path(project: &Option<String>) -> Result<Option<PathBuf>> {
    match project {
        Some(p) => {
            let path =
                std::fs::canonicalize(p).context(format!("Cannot resolve project path: {}", p))?;
            Ok(Some(path))
        }
        None => Ok(None),
    }
}

/// Print cards as a compact aligned table.
fn print_table(cards: &[Card], col_names: &std::collections::HashMap<&str, &str>) {
    if cards.is_empty() {
        println!("No cards found.");
        return;
    }

    // Calculate column widths
    let id_w = 10.max(cards.iter().map(|c| c.id.len()).max().unwrap_or(3));
    let title_w = 6.max(cards.iter().map(|c| c.title.len()).max().unwrap_or(5));
    let col_w: usize = cards
        .iter()
        .map(|c| {
            col_names
                .get(c.column_id.as_str())
                .map_or("?", |s| *s)
                .len()
        })
        .max()
        .unwrap_or(3);
    let col_w = 6.max(col_w);
    let pri_w = 6.max(
        cards
            .iter()
            .map(|c| c.priority.to_string().len())
            .max()
            .unwrap_or(5),
    );

    // Header
    println!(
        "{:<id_w$} {:<title_w$} {:<col_w$} {:<pri_w$}",
        "ID", "TITLE", "COLUMN", "PRIORITY"
    );
    println!(
        "{:-<id_w$} {:-<title_w$} {:-<col_w$} {:-<pri_w$}",
        "", "", "", ""
    );

    for card in cards {
        let col_name = col_names.get(card.column_id.as_str()).map_or("?", |s| *s);
        println!(
            "{:<id_w$} {:<title_w$} {:<col_w$} {:<pri_w$}",
            truncate(&card.id, id_w),
            truncate(&card.title, title_w),
            truncate(col_name, col_w),
            truncate(&card.priority.to_string(), pri_w),
        );
    }

    println!("\nTotal: {} card(s)", cards.len());
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}..", &s[..max - 2])
    }
}
