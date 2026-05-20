use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

use crate::board::store::Store;
use crate::kanban::config::{cards_dir, db_path, is_initialized};
use crate::markdown::writer::{card_file_path, sync_card};
use crate::MoveArgs;

/// Move a card to a different column.
pub fn transition(args: &MoveArgs) -> Result<()> {
    let project_path = resolve_project_path()?;

    let (mut store, board, cards_dir_path) = if let Some(ref p) = project_path {
        if !is_initialized(p) {
            anyhow::bail!("Project at {:?} is not initialized. Run `kanban init` first.", p);
        }
        let db = db_path(p);
        let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;
        let board = store.get_board(p.to_string_lossy().as_ref())
            .context(format!("Failed to open board at {:?}", p))?;
        (store, board, cards_dir(p))
    } else {
        let cwd = std::env::current_dir()?;
        if !is_initialized(&cwd) {
            anyhow::bail!("No project specified and no kanban board in current directory. Run `kanban init` first.");
        }
        let db = db_path(&cwd);
        let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;
        let board = store.get_board(cwd.to_string_lossy().as_ref())
            .context(format!("Failed to open board at {:?}", cwd))?;
        (store, board, cards_dir(&cwd))
    };

    // Resolve target column
    let target_col_name = args.column.clone();
    let column_id = board.columns.iter().find(|c| c.name == target_col_name)
        .ok_or_else(|| anyhow!("Unknown column: {}. Available: {}", target_col_name,
            board.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")))?
        .id
        .clone();

    // Get the card to know its current file path
    let card = store.get_card(&args.card_id)
        .context(format!("Card not found: {}", args.card_id))?;

    // Transition in SQLite
    store.transition_card(&args.card_id, &column_id)
        .context("Failed to transition card in database")?;

    // Update markdown file
    let card_path = card_file_path(&card.id, &cards_dir_path);
    if card_path.exists() {
        let mut updated_card = card.clone();
        updated_card.column_id = column_id.clone();
        sync_card(&updated_card, &cards_dir_path).context("Failed to update card markdown file")?;
    }

    let col_name = board.columns.iter().find(|c| c.id == column_id)
        .map(|c| c.name.as_str())
        .unwrap_or("???");

    println!("Moved card {} to {}", args.card_id, col_name);
    println!("  Title: {}", card.title);
    println!("  Priority: {}", card.priority);
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
