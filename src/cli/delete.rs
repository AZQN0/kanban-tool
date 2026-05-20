use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::board::store::Store;
use crate::kanban::config::{cards_dir, db_path, is_initialized};
use crate::persistence::delete_card_with_markdown;
use crate::DeleteArgs;

/// Delete a card by ID.
pub fn delete(args: &DeleteArgs) -> Result<()> {
    let project_path = resolve_project_path()?;

    let (mut store, cards_dir_path) = if let Some(ref p) = project_path {
        if !is_initialized(p) {
            anyhow::bail!(
                "Project at {:?} is not initialized. Run `kanban init` first.",
                p
            );
        }
        let db = db_path(p);
        let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;
        (store, cards_dir(p))
    } else {
        let cwd = std::env::current_dir()?;
        if !is_initialized(&cwd) {
            anyhow::bail!("No project specified and no kanban board in current directory. Run `kanban init` first.");
        }
        let db = db_path(&cwd);
        let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;
        (store, cards_dir(&cwd))
    };

    let card = delete_card_with_markdown(&mut store, &args.card_id, &cards_dir_path)
        .context("Failed to delete card and markdown export")?;

    println!("Deleted card: {}", card.id);
    println!("  Title: {}", card.title);
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
