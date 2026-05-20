use rust_mcp_sdk::{
    auth::AuthInfo,
    macros::{mcp_tool, JsonSchema},
    schema::{schema_utils::CallToolError, CallToolResult, TextContent},
    tool_box,
};
use serde::Serialize;

use crate::board::card::Card;
use crate::board::card::Priority;
use crate::board::column::Column;
use crate::board::store::Store;
use crate::kanban::config::{cards_dir, db_path, is_initialized};
use crate::kanban::init::init_board;
use crate::persistence::{
    card_export_file, create_card_with_markdown, delete_card_with_markdown,
    move_card_with_markdown, update_card_with_markdown, CardPatch,
};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize)]
struct CardSummaryResponse {
    card_id: String,
    title: String,
    column: String,
}

#[derive(Serialize)]
struct CardListItemResponse {
    id: String,
    title: String,
    column: String,
    priority: String,
}

#[derive(Serialize)]
struct CardSearchItemResponse {
    id: String,
    title: String,
    column: String,
    priority: String,
    description: String,
}

#[derive(Serialize)]
struct BoardInitResponse {
    board_id: String,
    name: String,
    columns: Vec<String>,
}

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

        let mut card = Card::new(
            &board_id,
            &column_id,
            &self.title,
            self.description.as_deref().unwrap_or(""),
            priority,
            self.labels.clone().unwrap_or_default(),
            PathBuf::new(),
        );
        card.card_file = card_export_file(&card.id);

        let card_id = create_card_with_markdown(&mut store, &card, &cards_dir(&project_path))
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let col_name =
            get_column_name(&store, &project_path, &column_id).unwrap_or_else(|| "?".to_string());

        json_text_content(&CardSummaryResponse {
            card_id,
            title: self.title.clone(),
            column: col_name,
        })
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
    pub project: Option<String>,
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
        let project_path = resolve_project(&self.project)?;
        if !is_initialized(&project_path) {
            return Err(CallToolError::from_message(format!(
                "Project at {:?} is not initialized.",
                project_path
            )));
        }
        let db = db_path(&project_path);
        let mut store = Store::open(&db).map_err(|e| CallToolError::from_message(e.to_string()))?;

        let mut new_column_id: Option<String> = None;

        if let Some(ref col_name) = self.column {
            let board = store
                .get_board(&project_path.to_string_lossy())
                .map_err(|e| CallToolError::from_message(e.to_string()))?;
            let col = resolve_column(&board, &Some(col_name.clone()))
                .map_err(|e| CallToolError::from_message(e.to_string()))?;
            new_column_id = Some(col);
        }

        let new_priority = match &self.priority {
            Some(priority) => Some(resolve_priority(&Some(priority.clone()))?),
            None => None,
        };

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

        json_text_content(&CardSummaryResponse {
            card_id: updated.id,
            title: updated.title,
            column: col_name,
        })
    }
}

#[mcp_tool(
    name = "delete_card",
    description = "Delete a kanban card by its ID. Returns confirmation."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct DeleteCardTool {
    pub card_id: String,
    #[serde(default)]
    pub project: Option<String>,
}

impl DeleteCardTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let project_path = resolve_project(&self.project)?;
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

        if let Some(priority) = &self.priority {
            resolve_priority(&Some(priority.clone()))?;
        }

        let column_id = match &self.column {
            Some(column) => Some(resolve_column(&board, &Some(column.clone()))?),
            None => None,
        };

        let cards = store
            .list_cards(
                &board.id,
                column_id.as_deref(),
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

        let summaries: Vec<CardListItemResponse> = cards
            .iter()
            .map(|c| {
                let col = col_names.get(c.column_id.as_str()).map_or("?", |s| *s);
                CardListItemResponse {
                    id: c.id.clone(),
                    title: c.title.clone(),
                    column: col.to_string(),
                    priority: c.priority.to_string(),
                }
            })
            .collect();

        json_text_content(&summaries)
    }
}

#[mcp_tool(
    name = "transition_card",
    description = "Move a kanban card to a different column. Returns the updated card info."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct TransitionCardTool {
    pub card_id: String,
    #[serde(default)]
    pub project: Option<String>,
    pub column: String,
}

impl TransitionCardTool {
    fn call_tool(&self, _auth: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let project_path = resolve_project(&self.project)?;
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

        json_text_content(&CardSummaryResponse {
            card_id: self.card_id.clone(),
            title: updated.title,
            column: self.column.clone(),
        })
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

        let summaries: Vec<CardSearchItemResponse> = cards
            .iter()
            .map(|c| {
                let col = col_names.get(c.column_id.as_str()).map_or("?", |s| *s);
                CardSearchItemResponse {
                    id: c.id.clone(),
                    title: c.title.clone(),
                    column: col.to_string(),
                    priority: c.priority.to_string(),
                    description: c.description.clone(),
                }
            })
            .collect();

        json_text_content(&summaries)
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
                    Ok(board) => json_text_content(&BoardInitResponse {
                        board_id: board.id,
                        name: board.name,
                        columns: board.columns.into_iter().map(|c| c.name).collect(),
                    }),
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

                let col_id = resolve_column(&board, &Some(col_name.to_string()))?;

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
                    .execute("DELETE FROM columns WHERE id = ?", [&col_id])
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

fn json_text_content<T: Serialize>(value: &T) -> Result<CallToolResult, CallToolError> {
    let json = serde_json::to_string(value).map_err(|e| {
        CallToolError::from_message(format!("Failed to serialize MCP response: {}", e))
    })?;
    Ok(CallToolResult::text_content(vec![TextContent::from(json)]))
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_mcp_sdk::schema::ContentBlock;
    use serde_json::Value;
    use std::fs;
    use std::path::Path;

    struct TestProject {
        path: PathBuf,
    }

    impl TestProject {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("kanban_mcp_schema_test_{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            init_board(&path).unwrap();
            Self { path }
        }

        fn project_arg(&self) -> String {
            self.path.to_string_lossy().into_owned()
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn text(result: CallToolResult) -> String {
        match result.content.into_iter().next().unwrap() {
            ContentBlock::TextContent(content) => content.text,
            other => panic!("expected text content, got {other:?}"),
        }
    }

    fn create_card(project: &Path, title: &str) -> String {
        create_card_with_labels(project, title, None)
    }

    fn create_card_with_labels(project: &Path, title: &str, labels: Option<Vec<String>>) -> String {
        let result = CreateCardTool {
            project: project.to_string_lossy().into_owned(),
            title: title.to_string(),
            description: None,
            column: None,
            priority: None,
            labels,
        }
        .call_tool(None)
        .unwrap();

        serde_json::from_str::<Value>(&text(result)).unwrap()["card_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn get_card(project: &Path, card_id: &str) -> Card {
        Store::open(&db_path(project))
            .unwrap()
            .get_card(card_id)
            .unwrap()
    }

    #[test]
    fn update_card_without_labels_updates_title() {
        let project = TestProject::new();
        let card_id = create_card(&project.path, "Old title");

        let result = UpdateCardTool {
            card_id: card_id.clone(),
            project: Some(project.project_arg()),
            title: Some("New title".to_string()),
            description: None,
            column: None,
            priority: None,
            labels: None,
        }
        .call_tool(None)
        .unwrap();

        let body: Value = serde_json::from_str(&text(result)).unwrap();
        assert_eq!(body["card_id"], card_id);
        assert_eq!(body["title"], "New title");
        assert_eq!(get_card(&project.path, &card_id).title, "New title");
    }

    #[test]
    fn update_card_without_labels_preserves_existing_labels() {
        let project = TestProject::new();
        let card_id = create_card_with_labels(
            &project.path,
            "Labeled title",
            Some(vec!["backend".to_string(), "security".to_string()]),
        );

        UpdateCardTool {
            card_id: card_id.clone(),
            project: Some(project.project_arg()),
            title: Some("Renamed labeled title".to_string()),
            description: None,
            column: None,
            priority: None,
            labels: None,
        }
        .call_tool(None)
        .unwrap();

        let stored = get_card(&project.path, &card_id);
        assert_eq!(stored.title, "Renamed labeled title");
        assert_eq!(
            stored.labels,
            vec!["backend".to_string(), "security".to_string()]
        );
    }

    #[test]
    fn card_titles_with_quotes_and_newlines_produce_valid_json() {
        let project = TestProject::new();
        let title = "Needs \"escaping\"\nand newlines";

        let result = CreateCardTool {
            project: project.project_arg(),
            title: title.to_string(),
            description: None,
            column: None,
            priority: None,
            labels: None,
        }
        .call_tool(None)
        .unwrap();

        let body: Value = serde_json::from_str(&text(result)).unwrap();
        assert_eq!(body["title"], title);
    }

    #[test]
    fn delete_card_with_project_deletes_from_requested_project() {
        let project_a = TestProject::new();
        let project_b = TestProject::new();
        let keep_id = create_card(&project_a.path, "Keep");
        let delete_id = create_card(&project_b.path, "Delete");

        DeleteCardTool {
            card_id: delete_id.clone(),
            project: Some(project_b.project_arg()),
        }
        .call_tool(None)
        .unwrap();

        assert!(Store::open(&db_path(&project_b.path))
            .unwrap()
            .get_card(&delete_id)
            .is_err());
        assert_eq!(get_card(&project_a.path, &keep_id).title, "Keep");
    }

    #[test]
    fn unknown_column_returns_available_columns() {
        let project = TestProject::new();
        let card_id = create_card(&project.path, "Move me");

        let err = TransitionCardTool {
            card_id,
            project: Some(project.project_arg()),
            column: "missing".to_string(),
        }
        .call_tool(None)
        .unwrap_err();
        let message = err.to_string();

        assert!(message.contains("Unknown column 'missing'"), "{message}");
        assert!(
            message.contains("Available: backlog, todo, in_progress, review, done"),
            "{message}"
        );
    }

    #[test]
    fn list_cards_unknown_column_returns_available_columns() {
        let project = TestProject::new();
        create_card(&project.path, "List me");

        let err = ListCardsTool {
            project: Some(project.project_arg()),
            column: Some("missing".to_string()),
            priority: None,
            labels: None,
            limit: None,
            offset: None,
        }
        .call_tool(None)
        .unwrap_err();
        let message = err.to_string();

        assert!(message.contains("Unknown column 'missing'"), "{message}");
        assert!(
            message.contains("Available: backlog, todo, in_progress, review, done"),
            "{message}"
        );
    }

    #[test]
    fn remove_column_unknown_column_returns_available_columns() {
        let project = TestProject::new();

        let err = ManageBoardTool {
            action: "remove_column".to_string(),
            project: Some(project.project_arg()),
            column_name: Some("missing".to_string()),
        }
        .call_tool(None)
        .unwrap_err();
        let message = err.to_string();

        assert!(message.contains("Unknown column 'missing'"), "{message}");
        assert!(
            message.contains("Available: backlog, todo, in_progress, review, done"),
            "{message}"
        );
    }

    #[test]
    fn update_card_rejects_invalid_priority() {
        let project = TestProject::new();
        let card_id = create_card(&project.path, "Priority");

        let err = UpdateCardTool {
            card_id,
            project: Some(project.project_arg()),
            title: None,
            description: None,
            column: None,
            priority: Some("not-a-priority".to_string()),
            labels: None,
        }
        .call_tool(None)
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("Unknown priority 'not-a-priority'"),
            "{}",
            err
        );
    }
}
