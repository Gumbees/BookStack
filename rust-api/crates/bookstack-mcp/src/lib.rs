//! Built-in `bookstack-mcp` server.
//!
//! Implements the Model Context Protocol (JSON-RPC 2.0 over the Streamable
//! HTTP transport) directly on top of the core services — no HTTP round-trip
//! through the REST API. Mounted by `bookstack-api` at `POST /mcp`, protected
//! by the same JWT / `Authorization: Token id:secret` auth as the REST API.
//!
//! The tool surface, argument conventions, response formats, and initialize
//! instructions are ported from `bees-roadhouse/bookstack-mcp` (v0.13.0) so
//! clients configured for that server work against this one for every
//! feature this backend supports. Ported conventions include:
//! `page_id`/`book_id`-style argument names, required meaningful
//! descriptions on shelf/book/chapter creation (with placeholder rejection),
//! duplicate-title stripping, slim text success responses carrying clickable
//! URLs, the live structure tree embedded in initialize instructions, the
//! surgical editing suite (`edit_page`, `replace_section`, `insert_after`,
//! `append_to_page`), `directory`, exports, comments, the recycle bin, and
//! `_meta.time` on every tools/call response.
//!
//! Not ported (backend features absent from the rewrite): attachments,
//! images/staging uploads, per-content permissions, audit log, and the
//! semantic-search/embedding suite. Those tools are simply not registered,
//! mirroring upstream's conditional registration pattern.

mod structure;
mod tools;

use serde_json::{json, Value};

use bookstack_collab::CollabEngine;
use bookstack_core::models::{AuthUser, Role};
use bookstack_core::Core;
use bookstack_semantic::SemanticEngine;
use std::sync::Arc;

/// Per-request acting context: the authenticated user, the org the request
/// operates in, and the user's role inside that org.
#[derive(Debug, Clone)]
pub struct McpCtx {
    pub user: AuthUser,
    pub org_id: i64,
    pub org_role: Role,
}

pub const SERVER_NAME: &str = "bookstack-mcp";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub struct McpServer {
    pub core: Core,
    pub collab: Arc<CollabEngine>,
    pub semantic: Option<Arc<SemanticEngine>>,
}

impl McpServer {
    pub fn new(
        core: Core,
        collab: Arc<CollabEngine>,
        semantic: Option<Arc<SemanticEngine>>,
    ) -> McpServer {
        McpServer { core, collab, semantic }
    }

    /// Handle one JSON-RPC message. Returns `None` for notifications
    /// (which get an HTTP 202 with no body).
    pub async fn handle(&self, ctx: &McpCtx, message: Value) -> Option<Value> {
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        // Notifications carry no id and never get a response.
        let Some(id) = id else {
            return None;
        };

        let response = match method {
            "initialize" => json_rpc_result(id, self.initialize(ctx, &params).await),
            "ping" => json_rpc_result(id, json!({})),
            "tools/list" => {
                json_rpc_result(id, json!({ "tools": tools::definitions(self.semantic.is_some()) }))
            }
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let result = tools::call(self, ctx, name, &args).await;

                // Every tools/call result carries `_meta.time` (same shape as
                // upstream issue #67) so sessions can reason about
                // "today / yesterday" without re-deriving conversions.
                let meta = json!({ "time": time_block(&self.core.config.timezone) });
                let tool_result = match result {
                    Ok(text) => json!({
                        "content": [{ "type": "text", "text": text }],
                        "_meta": meta,
                    }),
                    Err(e) => json!({
                        "content": [{ "type": "text", "text": format!("Error: {e}") }],
                        "isError": true,
                        "_meta": meta,
                    }),
                };
                json_rpc_result(id, tool_result)
            }
            // Friendly empty responses for optional capabilities some clients probe.
            "resources/list" => json_rpc_result(id, json!({ "resources": [] })),
            "resources/templates/list" => json_rpc_result(id, json!({ "resourceTemplates": [] })),
            "prompts/list" => json_rpc_result(id, json!({ "prompts": [] })),
            _ => json_rpc_error(id, -32601, &format!("method not found: {method}")),
        };
        Some(response)
    }

    async fn initialize(&self, ctx: &McpCtx, params: &Value) -> Value {
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
                "title": "BookStack MCP",
                "version": SERVER_VERSION,
            },
            "instructions": structure::instructions(&self.core, ctx.org_id).await,
        })
    }
}

fn json_rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn json_rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Time block matching upstream's `_meta.time` shape.
fn time_block(timezone: &str) -> Value {
    let now = chrono::Utc::now();
    let tz: chrono_tz::Tz = timezone.parse().unwrap_or(chrono_tz::UTC);
    let local = now.with_timezone(&tz);
    json!({
        "now_unix": now.timestamp(),
        "now_utc": now.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "now_local": local.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        "now_human": local.format("%A, %B %-d, %Y at %-I:%M %p %Z").to_string(),
        "timezone": timezone,
        "timezone_source": if timezone == "UTC" { "default" } else { "env" },
    })
}
