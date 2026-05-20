use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

use crate::board::card::Priority;
use crate::board::store::Store;
use crate::kanban::config::{cards_dir, db_path, is_initialized};
use crate::persistence::{update_card_with_markdown, CardPatch};
use crate::UpdateArgs;

/// Update a card's fields.
pub fn update(args: &UpdateArgs) -> Result<()> {
    let project_path = resolve_project_path()?;

    let (mut store, board, cards_dir_path) = if let Some(ref p) = project_path {
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

    // Resolve column if provided
    let new_column_id = if let Some(ref col_name) = args.column {
        let col = board
            .columns
            .iter()
            .find(|c| c.name == *col_name)
            .ok_or_else(|| {
                anyhow!(
                    "Unknown column: {}. Available: {}",
                    col_name,
                    board
                        .columns
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
        Some(col.id.clone())
    } else {
        None
    };

    // Resolve priority if provided
    let new_priority = if let Some(ref p) = args.priority {
        Some(Priority::from_str(p).ok_or_else(|| {
            anyhow!(
                "Unknown priority: {}. Use: backlog, low, medium, high, urgent",
                p
            )
        })?)
    } else {
        None
    };

    // Labels: replace or keep existing
    let new_labels = if let Some(ref labels) = args.label {
        if labels.is_empty() {
            Some(vec![] as Vec<String>)
        } else {
            Some(labels.clone())
        }
    } else {
        None
    };

    let card = update_card_with_markdown(
        &mut store,
        &args.card_id,
        CardPatch {
            title: args.title.clone(),
            description: args.description.clone(),
            column_id: new_column_id,
            priority: new_priority,
            labels: new_labels,
        },
        &cards_dir_path,
    )
    .context("Failed to update card and markdown export")?;

    // Resolve new column name
    let col_name = board
        .columns
        .iter()
        .find(|c| c.id == card.column_id)
        .map(|c| c.name.as_str())
        .unwrap_or("?");

    println!("Updated card: {}", card.id);
    println!("  Title:      {}", card.title);
    println!("  Column:     {}", col_name);
    println!("  Priority:   {}", card.priority);
    if !card.labels.is_empty() {
        println!("  Labels:     {}", card.labels.join(", "));
    }
    Ok(())
}

/// Try to find a .kanban directory — start from cwd, walk up the tree.
fn resolve_project_path() -> Result<Option<PathBuf>> {
    let cwd = std::env::current_dir()?;
    if is_initialized(&cwd) {
        return Ok(Some(cwd));
    }
    for ancestor in cwd.ancestors().skip(1) {
        if is_initialized(ancestor) {
            return Ok(Some(ancestor.to_path_buf()));
        }
    }
    Ok(None)
}
