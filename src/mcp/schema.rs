use rust_mcp_sdk::{
    auth::AuthInfo,
    macros::{mcp_tool, JsonSchema},
    schema::{schema_utils::CallToolError, CallToolResult, TextContent},
    tool_box,
};

use crate::board::card::Card;
use crate::board::card::Priority;
use crate::board::column::Column;
use crate::board::store::Store;
use crate::kanban::config::{cards_dir, db_path, is_initialized};
use crate::kanban::init::init_board;
use crate::persistence::{
    create_card_with_markdown, delete_card_with_markdown, move_card_with_markdown,
    update_card_with_markdown, CardPatch,
};
use std::collections::HashMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Tool parameter structs
// ─────────────────────────────────────────────────────────────────────────────

#[mcp_tool(
    name = "create_card",
    description = "Create a new kanban card in a project's board. Returns the card ID."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct CreateCardTool {
    pub project: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub column: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
}

impl CreateCardTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let project_path = resolve_project(&Some(self.project.clone()))?;
        if !is_initialized(&project_path) {
            return Err(CallToolError::from_message(format!(
                "Project at {:?} is not initialized. Run `kanban init` first.",
                project_path
            )));
        }
        let (board_id, column_id, priority) = {
            let db = db_path(&project_path);
            let store = Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;
            let board = store
                .get_board(&project_path.to_string_lossy())
                .map_err(|e| CallToolError::from_message(e.to_string()))?;

            let column_id = resolve_column(&board, &self.column)?;
            let priority = resolve_priority(&self.priority)?;
            (board.id, column_id, priority)
        };

        let mut store = Store::open(&db_path(&project_path))
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let card = Card::new(
            &board_id,
            &column_id,
            &self.title,
            self.description.as_deref().unwrap_or(""),
            priority,
            self.labels.clone().unwrap_or_default(),
            PathBuf::from(format!("{}.md", uuid::Uuid::new_v4().to_string())),
        );

        let card_id = create_card_with_markdown(&mut store, &card, &cards_dir(&project_path))
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let col_name =
            get_column_name(&store, &project_path, &column_id).unwrap_or_else(|| "?".to_string());

        Ok(CallToolResult::text_content(vec![TextContent::from(
            format!(
                r#"{{"card_id": "{}", "title": "{}", "column": "{}"}}"#,
                card_id, self.title, col_name
            ),
        )]))
    }
}

#[mcp_tool(
    name = "get_card",
    description = "Get a kanban card by its ID. Returns the full card data including description."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct GetCardTool {
    pub card_id: String,
    #[serde(default)]
    pub project: Option<String>,
}

impl GetCardTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let project_path = resolve_project(&self.project)?;
        if !is_initialized(&project_path) {
            return Err(CallToolError::from_message(format!(
                "Project at {:?} is not initialized.",
                project_path
            )));
        }
        let db = db_path(&project_path);
        let store = Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;
        let card = store
            .get_card(&self.card_id)
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let json = serde_json::to_string_pretty(&card)
            .map_err(|e| CallToolError::from_message(e.to_string()))?;
        Ok(CallToolResult::text_content(vec![TextContent::from(json)]))
    }
}

#[mcp_tool(
    name = "update_card",
    description = "Update a kanban card's fields. Returns updated card summary."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct UpdateCardTool {
    pub card_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub column: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
}

impl UpdateCardTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let project_path = resolve_project(&None)?;
        if !is_initialized(&project_path) {
            return Err(CallToolError::from_message(format!(
                "Project at {:?} is not initialized.",
                project_path
            )));
        }
        let db = db_path(&project_path);
        let mut store = Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;
        let card = store
            .get_card(&self.card_id)
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let mut new_column_id: Option<String> = None;

        if let Some(ref col_name) = self.column {
            let board = store
                .get_board(&project_path.to_string_lossy())
                .map_err(|e| CallToolError::from_message(e.to_string()))?;
            let col = resolve_column(&board, &Some(col_name.clone()))
                .map_err(|e| CallToolError::from_message(e.to_string()))?;
            new_column_id = Some(col);
        }

        let new_priority = self
            .priority
            .as_ref()
            .map(|p| resolve_priority(&Some(p.clone())).unwrap_or(card.priority.clone()));

        let updated = update_card_with_markdown(
            &mut store,
            &self.card_id,
            CardPatch {
                title: self.title.clone(),
                description: self.description.clone(),
                column_id: new_column_id,
                priority: new_priority,
                labels: self.labels.clone(),
            },
            &cards_dir(&project_path),
        )
        .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let col_name = get_column_name(&store, &project_path, &updated.column_id)
            .unwrap_or_else(|| "?".to_string());

        Ok(CallToolResult::text_content(vec![TextContent::from(
            format!(
                r#"{{"card_id": "{}", "title": "{}", "column": "{}"}}"#,
                updated.id, updated.title, col_name
            ),
        )]))
    }
}

#[mcp_tool(
    name = "delete_card",
    description = "Delete a kanban card by its ID. Returns confirmation."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct DeleteCardTool {
    pub card_id: String,
}

impl DeleteCardTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let project_path = resolve_project(&None)?;
        if !is_initialized(&project_path) {
            return Err(CallToolError::from_message(format!(
                "Project at {:?} is not initialized.",
                project_path
            )));
        }
        let db = db_path(&project_path);
        let mut store = Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;

        delete_card_with_markdown(&mut store, &self.card_id, &cards_dir(&project_path))
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        Ok(CallToolResult::text_content(vec![TextContent::from(
            format!("Card {} deleted successfully.", self.card_id),
        )]))
    }
}

#[mcp_tool(
    name = "list_cards",
    description = "List kanban cards with optional filters. Returns an array of card summaries."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct ListCardsTool {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub column: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

impl ListCardsTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let project_path = resolve_project(&self.project)?;
        if !is_initialized(&project_path) {
            return Err(CallToolError::from_message(format!(
                "Project at {:?} is not initialized.",
                project_path
            )));
        }
        let db = db_path(&project_path);
        let store = Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;
        let board = store
            .get_board(&project_path.to_string_lossy())
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let column_id = self
            .column
            .as_ref()
            .and_then(|name| Column::find_by_name(&board.columns, name).map(|c| c.id.as_str()));

        let cards = store
            .list_cards(
                &board.id,
                column_id,
                self.priority.as_deref(),
                self.labels.as_deref(),
                "created",
            )
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let cards: Vec<_> = match (
            self.offset.map(|v| v as usize),
            self.limit.map(|v| v as usize),
        ) {
            (Some(off), Some(lim)) => cards.into_iter().skip(off).take(lim).collect(),
            (Some(off), None) => cards.into_iter().skip(off).collect(),
            (None, Some(lim)) => cards.into_iter().take(lim).collect(),
            (None, None) => cards,
        };

        let col_names: HashMap<&str, &str> = board
            .columns
            .iter()
            .map(|c| (c.id.as_str(), c.name.as_str()))
            .collect();

        let summaries: Vec<String> = cards
            .iter()
            .map(|c| {
                let col = col_names.get(c.column_id.as_str()).map_or("?", |s| *s);
                format!(
                    r#"{{"id": "{}", "title": "{}", "column": "{}", "priority": "{}"}}"#,
                    c.id, c.title, col, c.priority
                )
            })
            .collect();

        Ok(CallToolResult::text_content(vec![TextContent::from(
            format!("[{}]", summaries.join(", ")),
        )]))
    }
}

#[mcp_tool(
    name = "transition_card",
    description = "Move a kanban card to a different column. Returns the updated card info."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct TransitionCardTool {
    pub card_id: String,
    pub column: String,
}

impl TransitionCardTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let project_path = resolve_project(&None)?;
        if !is_initialized(&project_path) {
            return Err(CallToolError::from_message(format!(
                "Project at {:?} is not initialized.",
                project_path
            )));
        }
        let db = db_path(&project_path);
        let mut store = Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;
        let board = store
            .get_board(&project_path.to_string_lossy())
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let column_id = resolve_column(&board, &Some(self.column.clone()))
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let updated = move_card_with_markdown(
            &mut store,
            &self.card_id,
            &column_id,
            &cards_dir(&project_path),
        )
        .map_err(|e| CallToolError::from_message(e.to_string()))?;

        Ok(CallToolResult::text_content(vec![TextContent::from(
            format!(
                r#"{{"card_id": "{}", "title": "{}", "column": "{}"}}"#,
                self.card_id, updated.title, self.column
            ),
        )]))
    }
}

#[mcp_tool(
    name = "search_cards",
    description = "Search kanban cards by query string across title and description."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct SearchCardsTool {
    pub query: String,
    #[serde(default)]
    pub project: Option<String>,
}

impl SearchCardsTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let project_path = resolve_project(&self.project)?;
        if !is_initialized(&project_path) {
            return Err(CallToolError::from_message(format!(
                "Project at {:?} is not initialized.",
                project_path
            )));
        }
        let db = db_path(&project_path);
        let store = Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;
        let board = store
            .get_board(&project_path.to_string_lossy())
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let cards = store
            .search_cards(&board.id, &self.query)
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let col_names: HashMap<&str, &str> = board
            .columns
            .iter()
            .map(|c| (c.id.as_str(), c.name.as_str()))
            .collect();

        let summaries: Vec<String> = cards.iter().map(|c| {
            let col = col_names.get(c.column_id.as_str()).map_or("?", |s| *s);
            format!(
                r#"{{"id": "{}", "title": "{}", "column": "{}", "priority": "{}", "description": "{}"}}"#,
                c.id, c.title, col, c.priority, c.description
            )
        }).collect();

        Ok(CallToolResult::text_content(vec![TextContent::from(
            format!("[{}]", summaries.join(", ")),
        )]))
    }
}

#[mcp_tool(
    name = "manage_board",
    description = "Manage kanban boards: init a new board, add or remove columns."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct ManageBoardTool {
    pub action: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub column_name: Option<String>,
}

impl ManageBoardTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        match self.action.as_str() {
            "init" => {
                let project_path = resolve_project(&self.project)?;
                match init_board(&project_path) {
                    Ok(board) => Ok(CallToolResult::text_content(vec![TextContent::from(
                        format!(
                            r#"{{"board_id": "{}", "name": "{}", "columns": [{}]}}"#,
                            board.id,
                            board.name,
                            board
                                .columns
                                .iter()
                                .map(|c| format!("\"{}\"", c.name))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    )])),
                    Err(e) => Err(CallToolError::from_message(e.to_string())),
                }
            }
            "add_column" => {
                let project_path = resolve_project(&self.project)?;
                if !is_initialized(&project_path) {
                    return Err(CallToolError::from_message(format!(
                        "Project at {:?} is not initialized.",
                        project_path
                    )));
                }
                let db = db_path(&project_path);
                let mut store =
                    Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;
                let board = store
                    .get_board(&project_path.to_string_lossy())
                    .map_err(|e| CallToolError::from_message(e.to_string()))?;

                let name = self.column_name.as_deref().ok_or_else(|| {
                    CallToolError::from_message("column_name is required for add_column")
                })?;

                let max_order = board
                    .columns
                    .iter()
                    .map(|c| c.sort_order)
                    .max()
                    .unwrap_or(0);

                let new_col = Column::new(&board.id, name, max_order + 1);
                store
                    .add_column(&new_col)
                    .map_err(|e| CallToolError::from_message(e.to_string()))?;

                Ok(CallToolResult::text_content(vec![TextContent::from(
                    format!("Column '{}' added to board '{}'.", name, board.name),
                )]))
            }
            "remove_column" => {
                let project_path = resolve_project(&self.project)?;
                if !is_initialized(&project_path) {
                    return Err(CallToolError::from_message(format!(
                        "Project at {:?} is not initialized.",
                        project_path
                    )));
                }
                let db = db_path(&project_path);
                let store =
                    Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;
                let board = store
                    .get_board(&project_path.to_string_lossy())
                    .map_err(|e| CallToolError::from_message(e.to_string()))?;

                let col_name = self.column_name.as_deref().ok_or_else(|| {
                    CallToolError::from_message("column_name is required for remove_column")
                })?;

                let col = Column::find_by_name(&board.columns, col_name).ok_or_else(|| {
                    CallToolError::from_message(format!("Unknown column '{}'", col_name))
                })?;
                let col_id = &col.id;

                let cards = store
                    .list_cards(&board.id, Some(col_id.as_str()), None, None, "created")
                    .map_err(|e| CallToolError::from_message(e.to_string()))?;
                if !cards.is_empty() {
                    return Err(CallToolError::from_message(format!(
                        "Column '{}' has {} card(s). Move or delete them first.",
                        col_name,
                        cards.len()
                    )));
                }

                store
                    .conn
                    .execute("DELETE FROM columns WHERE id = ?", [col_id])
                    .map_err(|e| CallToolError::from_message(e.to_string()))?;

                Ok(CallToolResult::text_content(vec![TextContent::from(
                    format!("Column '{}' removed from board '{}'.", col_name, board.name),
                )]))
            }
            _ => Err(CallToolError::from_message(format!(
                "Unknown action: '{}'. Use 'init', 'add_column', or 'remove_column'.",
                self.action
            ))),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Enum that combines all tool variants
// ─────────────────────────────────────────────────────────────────────────────
tool_box!(
    KanbanTools,
    [
        CreateCardTool,
        GetCardTool,
        UpdateCardTool,
        DeleteCardTool,
        ListCardsTool,
        TransitionCardTool,
        SearchCardsTool,
        ManageBoardTool,
    ]
);

impl Default for KanbanTools {
    fn default() -> Self {
        KanbanTools::CreateCardTool(CreateCardTool {
            project: String::new(),
            title: String::new(),
            description: None,
            column: None,
            priority: None,
            labels: None,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Server handler
// ─────────────────────────────────────────────────────────────────────────────

use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{ListToolsResult, PaginatedRequestParams};
use rust_mcp_sdk::{schema::RpcError, McpServer};
use std::sync::Arc;

pub struct KanbanHandler;

#[async_trait]
impl ServerHandler for KanbanHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: KanbanTools::tools(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: rust_mcp_sdk::schema::CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<
        rust_mcp_sdk::schema::CallToolResult,
        rust_mcp_sdk::schema::schema_utils::CallToolError,
    > {
        let tool: KanbanTools = KanbanTools::try_from(params)
            .map_err(rust_mcp_sdk::schema::schema_utils::CallToolError::new)?;

        match tool {
            KanbanTools::CreateCardTool(t) => t.call_tool(None),
            KanbanTools::GetCardTool(t) => t.call_tool(None),
            KanbanTools::UpdateCardTool(t) => t.call_tool(None),
            KanbanTools::DeleteCardTool(t) => t.call_tool(None),
            KanbanTools::ListCardsTool(t) => t.call_tool(None),
            KanbanTools::TransitionCardTool(t) => t.call_tool(None),
            KanbanTools::SearchCardsTool(t) => t.call_tool(None),
            KanbanTools::ManageBoardTool(t) => t.call_tool(None),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn resolve_project(project: &Option<String>) -> Result<PathBuf, CallToolError> {
    let path = match project {
        Some(p) => std::fs::canonicalize(p).map_err(|e| {
            CallToolError::from_message(format!("Cannot resolve project path '{}': {}", p, e))
        })?,
        None => std::env::current_dir().map_err(|e| CallToolError::from_message(e.to_string()))?,
    };
    Ok(path)
}

fn resolve_column(
    board: &crate::board::Board,
    column_name: &Option<String>,
) -> Result<String, CallToolError> {
    let name = match column_name {
        Some(n) => n.as_str(),
        None => "todo",
    };
    Column::find_by_name(&board.columns, name)
        .map(|c| c.id.clone())
        .ok_or_else(|| {
            let available: Vec<_> = board.columns.iter().map(|c| c.name.as_str()).collect();
            CallToolError::from_message(format!(
                "Unknown column '{}'. Available: {}",
                name,
                available.join(", ")
            ))
        })
}

fn resolve_priority(priority: &Option<String>) -> Result<Priority, CallToolError> {
    let p = priority.as_deref().unwrap_or("medium");
    Priority::from_str(p).ok_or_else(|| {
        CallToolError::from_message(format!(
            "Unknown priority '{}'. Use: backlog, low, medium, high, urgent",
            p
        ))
    })
}

fn get_column_name(store: &Store, project_path: &PathBuf, column_id: &str) -> Option<String> {
    let board = store.get_board(&project_path.to_string_lossy()).ok()?;
    board
        .columns
        .iter()
        .find(|c| c.id == column_id)
        .map(|c| c.name.clone())
}
