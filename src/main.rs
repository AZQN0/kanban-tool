use anyhow::{Context, Result};
use clap::Parser;

mod board;
mod kanban;
mod markdown;
mod mcp;
mod cli;
mod tui;

#[derive(clap::Args, Debug)]
struct ListArgs {
    #[arg(long)]
    project: Option<String>,
    #[arg(long, value_parser = ["backlog", "todo", "in_progress", "review", "done"])]
    column: Option<String>,
    #[arg(long)]
    label: Option<Vec<String>>,
    #[arg(long, value_parser = ["backlog", "low", "medium", "high", "urgent"])]
    priority: Option<String>,
}

#[derive(clap::Args, Debug)]
struct CreateArgs {
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    column: Option<String>,
    #[arg(long, default_value = "backlog")]
    priority: String,
    #[arg(long)]
    label: Option<Vec<String>>,
}

#[derive(clap::Args, Debug)]
struct MoveArgs {
    card_id: String,
    column: String,
}

#[derive(clap::Args, Debug)]
struct SearchArgs {
    query: String,
    #[arg(long)]
    project: Option<String>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Initialize a new kanban board for a project
    Init {
        /// Path to the project directory (default: current directory)
        #[arg(default_value = ".")]
        path: String,
    },
    /// List cards with optional filters
    List(ListArgs),
    /// Create a new card
    Create(CreateArgs),
    /// Move a card to a different column
    Move(MoveArgs),
    /// Search cards by query
    Search(SearchArgs),
    /// Launch the terminal UI
    Board,
    /// Start the MCP server for coding agents
    Server,
}

#[derive(clap::Parser, Debug)]
#[command(name = "kanban", about = "A kanban board system for coding agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => {
            let project_path = std::fs::canonicalize(&path)
                .context(format!("Cannot resolve path: {}", path))?;
            let board = kanban::init::init_board(&project_path)?;
            println!("Initialized kanban board at: {}", project_path.display());
            println!("  Board ID: {}", board.id);
            println!("  Columns:  {}", board.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "));
        }
        Commands::List(args) => {
            cli::list::list(&args)?;
        }
        Commands::Create(args) => {
            cli::create::create(&args)?;
        }
        Commands::Move(args) => {
            cli::transition::transition(&args)?;
        }
        Commands::Search(args) => {
            cli::search::search(&args)?;
        }
        Commands::Board => {
            println!("TUI not yet implemented. Use `kanban list` or `kanban create` for now.");
        }
        Commands::Server => {
            println!("MCP server not yet implemented. Use CLI commands for now.");
        }
    }

    Ok(())
}
