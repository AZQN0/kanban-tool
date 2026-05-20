use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::board::store::Store;
use crate::kanban::config::{db_path, is_initialized};
use crate::GetArgs;

/// Get a card by ID and print its full details.
pub fn get(args: &GetArgs) -> Result<()> {
    let project_path = resolve_project_path()?;

    let (store, card) = if let Some(ref p) = project_path {
        if !is_initialized(p) {
            anyhow::bail!(
                "Project at {:?} is not initialized. Run `kanban init` first.",
                p
            );
        }
        let db = db_path(p);
        let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;
        let card = store
            .get_card(&args.card_id)
            .context(format!("Card not found: {}", args.card_id))?;
        (store, card)
    } else {
        let cwd = std::env::current_dir()?;
        if !is_initialized(&cwd) {
            anyhow::bail!("No project specified and no kanban board in current directory.");
        }
        let db = db_path(&cwd);
        let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;
        let card = store
            .get_card(&args.card_id)
            .context(format!("Card not found: {}", args.card_id))?;
        (store, card)
    };

    // Resolve column name
    let board = project_path
        .as_ref()
        .and_then(|p| store.get_board(p.to_string_lossy().as_ref()).ok());
    let col_name = board
        .as_ref()
        .and_then(|b| b.columns.iter().find(|c| c.id == card.column_id))
        .map(|c| c.name.as_str())
        .unwrap_or("?");

    // Print card details
    println!("Card: {}", card.id);
    println!("  Title:      {}", card.title);
    println!("  Column:     {}", col_name);
    println!("  Priority:   {}", card.priority);
    if !card.labels.is_empty() {
        println!("  Labels:     {}", card.labels.join(", "));
    }
    if !card.description.is_empty() {
        println!(
            "  Description:\n    {}",
            card.description.replace("\n", "\n    ")
        );
    }
    if !card.subtasks.is_empty() {
        println!("  Subtasks:   {}", card.subtasks.join(", "));
    }
    if let Some(ref parent) = card.parent_card_id {
        println!("  Parent:     {}", parent);
    }
    println!("  Created:    {}", card.created_at.to_rfc3339());
    println!("  Updated:    {}", card.updated_at.to_rfc3339());

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
