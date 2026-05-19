use anyhow::Result;
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
    Init { path: String },
    List(ListArgs),
    Create(CreateArgs),
    Move(MoveArgs),
    Search(SearchArgs),
    Board,
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
            todo!("Task 4: Implement init")
        }
        Commands::List(args) => {
            todo!("Task 5: Implement list")
        }
        Commands::Create(args) => {
            todo!("Task 5: Implement create")
        }
        Commands::Move(args) => {
            todo!("Task 5: Implement move")
        }
        Commands::Search(args) => {
            todo!("Task 5: Implement search")
        }
        Commands::Board => {
            todo!("Task 7: Launch TUI")
        }
        Commands::Server => {
            todo!("Task 6: Launch MCP server")
        }
    }
}
