use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

use crate::board::card::{Card, Priority};
use crate::board::store::Store;
use crate::kanban::config::{cards_dir, db_path, is_initialized};
use crate::persistence::{card_export_file, create_card_with_markdown};
use crate::CreateArgs;

/// Create a new card in a project's kanban board.
pub fn create(args: &CreateArgs) -> Result<()> {
    let project_path = resolve_project_path(&args.project)?;

    let (mut store, board, cards_dir_path) = if let Some(p) = &project_path {
        if !is_initialized(p) {
            anyhow::bail!(
                "Project at {:?} is not initialized. Run `kanban init` first.",
                p
            );
        }
        let db = db_path(p);
        let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;
        let board = store
            .get_board(p.to_string_lossy().as_ref())
            .context(format!("Failed to open board at {:?}", p))?;
        (store, board, cards_dir(p))
    } else {
        let cwd = std::env::current_dir()?;
        if !is_initialized(&cwd) {
            anyhow::bail!("No project specified and no kanban board in current directory. Run `kanban init` first.");
        }
        let db = db_path(&cwd);
        let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;
        let board = store
            .get_board(cwd.to_string_lossy().as_ref())
            .context(format!("Failed to open board at {:?}", cwd))?;
        (store, board, cards_dir(&cwd))
    };

    // Resolve column
    let column_id = match &args.column {
        Some(name) => {
            let col = board
                .columns
                .iter()
                .find(|c| c.name == *name)
                .ok_or_else(|| anyhow!("Unknown column: {}", name))?;
            col.id.clone()
        }
        None => {
            // Default to "todo" if available, otherwise first column
            board
                .columns
                .iter()
                .find(|c| c.name == "todo")
                .or_else(|| board.columns.first())
                .ok_or_else(|| anyhow!("No columns available on this board"))?
                .id
                .clone()
        }
    };

    // Resolve priority
    let priority = Priority::from_str(&args.priority).ok_or_else(|| {
        anyhow!(
            "Unknown priority: {}. Use: backlog, low, medium, high, urgent",
            args.priority
        )
    })?;

    // Build the card
    let mut card = Card::new(
        &board.id,
        &column_id,
        &args.title,
        args.description.as_deref().unwrap_or(""),
        priority,
        args.label.clone().unwrap_or_default(),
        PathBuf::new(),
    );
    card.card_file = card_export_file(&card.id);

    let card_id = create_card_with_markdown(&mut store, &card, &cards_dir_path)
        .context("Failed to create card and markdown export")?;

    let col_name = board
        .columns
        .iter()
        .find(|c| c.id == column_id)
        .map(|c| c.name.as_str())
        .unwrap_or("?");
    println!("Created card: {}", card_id);
    println!("  Title:     {}", card.title);
    println!("  Column:    {}", col_name);
    println!("  Priority:  {}", card.priority);
    if !card.labels.is_empty() {
        println!("  Labels:    {}", card.labels.join(", "));
    }
    Ok(())
}

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
