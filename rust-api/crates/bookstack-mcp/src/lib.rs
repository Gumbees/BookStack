//! Built-in `bookstack-mcp` server.
//!
//! Implements the Model Context Protocol (JSON-RPC 2.0 over the Streamable
//! HTTP transport) directly on top of the core services — no HTTP round-trip
//! through the REST API. Mounted by `bookstack-api` at `POST /mcp`, protected
//! by the same JWT / `Authorization: Token id:secret` auth as the REST API.
//!
//! Tool set mirrors the community `bookstack-mcp` server (list/get/create/
//! update/delete for shelves, books, chapters and pages, plus search and
//! system info), with tool-level permission enforcement based on the
//! authenticated user's role.

mod tools;

use serde_json::{json, Value};

use bookstack_core::models::AuthUser;
use bookstack_core::Core;
use bookstack_collab::CollabEngine;
use std::sync::Arc;

pub const SERVER_NAME: &str = "bookstack-mcp";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub struct McpServer {
    pub core: Core,
    pub collab: Arc<CollabEngine>,
}

impl McpServer {
    pub fn new(core: Core, collab: Arc<CollabEngine>) -> McpServer {
        McpServer { core, collab }
    }

    /// Handle one JSON-RPC message. Returns `None` for notifications
    /// (which get an HTTP 202 with no body).
    pub async fn handle(&self, user: &AuthUser, message: Value) -> Option<Value> {
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        // Notifications carry no id and never get a response.
        let Some(id) = id else {
            if method == "notifications/initialized" || method.starts_with("notifications/") {
                return None;
            }
            return None;
        };

        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools::definitions() })),
            "tools/call" => self.tools_call(user, &params).await,
            // Friendly empty responses for optional capabilities some clients probe.
            "resources/list" => Ok(json!({ "resources": [] })),
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            _ => Err((-32601, format!("method not found: {method}"))),
        };

        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message },
            }),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0]);
        let negotiated = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
            requested
        } else {
            SUPPORTED_PROTOCOL_VERSIONS[0]
        };
        json!({
            "protocolVersion": negotiated,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": SERVER_NAME,
                "title": "BookStack MCP Server (built-in)",
                "version": SERVER_VERSION,
            },
            "instructions": "BookStack knowledge management server. Content is organized as: Shelves > Books > Chapters > Pages. Use search_content to find content by keyword, or navigate the hierarchy with the list/get tools. Pages are written in Markdown.",
        })
    }

    async fn tools_call(&self, user: &AuthUser, params: &Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((-32602, "missing tool name".to_string()))?;
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

        match tools::call(self, user, name, &args).await {
            Ok(value) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&value).unwrap_or_default(),
                }],
                "isError": false,
            })),
            Err(tools::ToolError::UnknownTool) => {
                Err((-32602, format!("unknown tool: {name}")))
            }
            Err(tools::ToolError::Failed(message)) => Ok(json!({
                "content": [{ "type": "text", "text": message }],
                "isError": true,
            })),
        }
    }
}
