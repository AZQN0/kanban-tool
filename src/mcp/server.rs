use rust_mcp_sdk::{
    error::SdkResult,
    mcp_server::{server_runtime, McpServerOptions},
    schema::{
        Implementation, InitializeResult, ProtocolVersion, ServerCapabilities,
        ServerCapabilitiesTools,
    },
    McpServer, StdioTransport, ToMcpServerHandler, TransportOptions,
};

use super::schema::KanbanHandler;

pub async fn run_server() -> SdkResult<()> {
    let handler = KanbanHandler.to_mcp_server_handler();
    let transport = StdioTransport::new(TransportOptions::default())?;

    let server_details = InitializeResult {
        server_info: Implementation {
            name: "kanban".into(),
            version: "0.1.0".into(),
            title: Some("Kanban Board MCP Server".into()),
            description: Some(
                "A kanban board system for coding agents across multiple projects".into(),
            ),
            icons: vec![],
            website_url: Some("https://github.com/user/kanban-tool".into()),
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        protocol_version: ProtocolVersion::V2025_11_25.into(),
        instructions: Some("Kanban board tools for managing cards and boards.".into()),
        meta: None,
    };

    let server = server_runtime::create_server(McpServerOptions {
        transport,
        handler,
        server_details,
        task_store: None,
        client_task_store: None,
        message_observer: None,
    });

    server.start().await
}
