use anyhow::{bail, Context, Result};
use clap::Parser;
use std::net::IpAddr;

mod board;
mod cli;
mod kanban;
mod markdown;
mod mcp;
mod tui;
mod webui;

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

#[derive(clap::Args, Debug)]
struct GetArgs {
    card_id: String,
}

#[derive(clap::Args, Debug)]
struct DeleteArgs {
    card_id: String,
}

#[derive(clap::Args, Debug)]
struct UpdateArgs {
    card_id: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    column: Option<String>,
    #[arg(long, value_parser = ["backlog", "low", "medium", "high", "urgent"])]
    priority: Option<String>,
    #[arg(long)]
    label: Option<Vec<String>>,
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
    /// Get a card by ID
    Get(GetArgs),
    /// Update a card's fields
    Update(UpdateArgs),
    /// Delete a card
    Delete(DeleteArgs),
    /// Launch the terminal UI
    Board,
    /// Start the WebUI in the browser
    WebUI {
        /// Bind address (default: 127.0.0.1)
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
        /// Port to listen on (default: 9876)
        #[arg(long, default_value_t = 9876)]
        port: u16,
        /// Allow binding WebUI to non-loopback addresses
        #[arg(long)]
        allow_remote: bool,
    },
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
            let project_path =
                std::fs::canonicalize(&path).context(format!("Cannot resolve path: {}", path))?;
            let board = kanban::init::init_board(&project_path)?;
            println!("Initialized kanban board at: {}", project_path.display());
            println!("  Board ID: {}", board.id);
            println!(
                "  Columns:  {}",
                board
                    .columns
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
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
        Commands::Get(args) => {
            cli::get::get(&args)?;
        }
        Commands::Update(args) => {
            cli::update::update(&args)?;
        }
        Commands::Delete(args) => {
            cli::delete::delete(&args)?;
        }
        Commands::Board => {
            let project_path =
                std::fs::canonicalize(".").context("Cannot resolve current directory")?;
            tui::app::run(project_path)?;
        }
        Commands::WebUI {
            bind,
            port,
            allow_remote,
        } => {
            validate_webui_bind(&bind, allow_remote)?;

            #[cfg(feature = "webui")]
            {
                let project_path =
                    std::fs::canonicalize(".").context("Cannot resolve current directory")?;
                webui::server::run(project_path, &bind, port)?;
            }
            #[cfg(not(feature = "webui"))]
            {
                let _bind = bind;
                let _port = port;
                let _allow_remote = allow_remote;
                eprintln!("WebUI feature not enabled. Build with --features webui");
            }
        }
        Commands::Server => {
            println!("Starting Kanban MCP server on stdio...");
            let rt =
                tokio::runtime::Runtime::new()?.block_on(async { mcp::server::run_server().await });
            if let Err(e) = rt {
                eprintln!(
                    "MCP server error: {}",
                    e.rpc_error_message().unwrap_or(&e.to_string())
                );
            }
        }
    }

    Ok(())
}

fn validate_webui_bind(bind: &str, allow_remote: bool) -> Result<()> {
    if allow_remote {
        return Ok(());
    }

    if bind.eq_ignore_ascii_case("localhost") {
        return Ok(());
    }

    let ip: IpAddr = bind
        .parse()
        .with_context(|| format!("Invalid WebUI bind address: {}", bind))?;

    if ip.is_loopback() {
        return Ok(());
    }

    bail!(
        "Refusing to bind WebUI to non-loopback address '{}'. Use --allow-remote to expose unauthenticated WebUI routes.",
        bind
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_webui_bind_allows_loopback_addresses() {
        validate_webui_bind("127.0.0.1", false).unwrap();
        validate_webui_bind("::1", false).unwrap();
        validate_webui_bind("localhost", false).unwrap();
    }

    #[test]
    fn validate_webui_bind_rejects_remote_without_override() {
        let err = validate_webui_bind("0.0.0.0", false).unwrap_err();

        assert!(err
            .to_string()
            .contains("Refusing to bind WebUI to non-loopback address"));
        assert!(err.to_string().contains("--allow-remote"));
    }

    #[test]
    fn validate_webui_bind_allows_remote_with_override() {
        validate_webui_bind("0.0.0.0", true).unwrap();
    }
}
