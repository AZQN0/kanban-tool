use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::board::store::Store;
use crate::kanban::config::{db_path, is_initialized};
use crate::ListArgs;

/// List cards, optionally filtered by column, priority, labels, or project.
///
/// Prints a compact table to stdout.
pub fn list(args: &ListArgs) -> Result<()> {
    let project_path = resolve_project_path(&args.project)?;

    let board = if let Some(p) = &project_path {
        if !is_initialized(p) {
            anyhow::bail!("No kanban board found at {:?}", p);
        }
        let db = db_path(p);
        Store::open(&db)?.get_board(p.to_string_lossy().as_ref())
            .context(format!("Failed to open board at {:?}", p))?
    } else {
        // No project specified — list from current directory if initialized
        let cwd = std::env::current_dir()?;
        if !is_initialized(&cwd) {
            anyhow::bail!("No project specified and no kanban board in current directory");
        }
        let db = db_path(&cwd);
        Store::open(&db)?.get_board(cwd.to_string_lossy().as_ref())
            .context(format!("Failed to open board at {:?}", cwd))?
    };

    let board_id = &board.id;
    let column_id = args.column.as_ref().and_then(|name| {
        board.columns.iter().find(|c| c.name == *name).map(|c| c.id.as_str())
    });

    let cards = {
        let cwd = std::env::current_dir()?;
        let db_p = project_path.as_ref().unwrap_or(&cwd);
        if let Some(cid) = column_id {
            Store::open(&db_path(db_p))?
                .list_cards(board_id, Some(cid.as_ref()), args.priority.as_deref(), args.label.as_deref(), "created")
                .context("Failed to list cards")?
        } else {
            Store::open(&db_path(db_p))?
                .list_cards(board_id, None, args.priority.as_deref(), args.label.as_deref(), "created")
                .context("Failed to list cards")?
        }
    };

    // Build a lookup from column_id -> column name
    let col_names: std::collections::HashMap<&str, &str> = board.columns.iter()
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();

    print_table(&cards, &col_names);
    Ok(())
}

/// Resolve the project path from the optional --project argument.
fn resolve_project_path(project: &Option<String>) -> Result<Option<PathBuf>> {
    match project {
        Some(p) => {
            let path = std::fs::canonicalize(p)
                .context(format!("Cannot resolve project path: {}", p))?;
            Ok(Some(path))
        }
        None => Ok(None),
    }
}

/// Print cards as a compact aligned table.
fn print_table(cards: &[crate::board::card::Card], col_names: &std::collections::HashMap<&str, &str>) {
    if cards.is_empty() {
        println!("No cards found.");
        return;
    }

    // Calculate column widths
    let id_w = 10.max(cards.iter().map(|c| c.id.len()).max().unwrap_or(3));
    let title_w = 6.max(cards.iter().map(|c| c.title.len()).max().unwrap_or(5));
    let col_w: usize = cards.iter()
        .map(|c| col_names.get(c.column_id.as_str()).map_or("?", |s| *s).len())
        .max()
        .unwrap_or(3);
    let col_w = 6.max(col_w);
    let pri_w = 6.max(cards.iter().map(|c| c.priority.to_string().len()).max().unwrap_or(5));

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
