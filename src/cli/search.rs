use anyhow::{Context, Result};

use crate::board::card::Card;
use crate::board::store::Store;
use crate::kanban::config::{db_path, is_initialized};
use crate::SearchArgs;

/// Search cards by query string across title and description.
pub fn search(args: &SearchArgs) -> Result<()> {
    let project_path = if let Some(ref p) = args.project {
        let path = std::fs::canonicalize(p)
            .context(format!("Cannot resolve project path: {}", p))?;
        if !is_initialized(&path) {
            anyhow::bail!("Project at {:?} is not initialized. Run `kanban init` first.", path);
        }
        Some(path)
    } else {
        // Search current directory
        let cwd = std::env::current_dir()?;
        if !is_initialized(&cwd) {
            anyhow::bail!("No project specified and no kanban board in current directory.");
        }
        Some(cwd)
    };

    let db = db_path(&project_path.as_ref().unwrap());
    let store = Store::open(&db).context(format!("Failed to open database at {:?}", db))?;
    let board = store.get_board(project_path.as_ref().unwrap().to_string_lossy().as_ref())
        .context("Failed to open board")?;

    let cards = store.search_cards(&board.id, &args.query)
        .context("Search failed")?;

    // Build column name lookup
    let col_names: std::collections::HashMap<&str, &str> = board.columns.iter()
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();

    if cards.is_empty() {
        println!("No cards found matching \"{}\".", args.query);
        return Ok(());
    }

    println!("Search results for \"{}\":\n", args.query);
    print_table(&cards, &col_names);
    Ok(())
}

fn print_table(cards: &[Card], col_names: &std::collections::HashMap<&str, &str>) {
    let id_w = 10.max(cards.iter().map(|c| c.id.len()).max().unwrap_or(3));
    let title_w = 6.max(cards.iter().map(|c| c.title.len()).max().unwrap_or(5));
    let col_w: usize = cards.iter()
        .map(|c| col_names.get(c.column_id.as_str()).map_or("?", |s| *s).len())
        .max()
        .unwrap_or(3);
    let col_w = 6.max(col_w);
    let pri_w = 6.max(cards.iter().map(|c| c.priority.to_string().len()).max().unwrap_or(5));

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
    if s.len() <= max { s.to_string() } else { format!("{}..", &s[..max - 2]) }
}
