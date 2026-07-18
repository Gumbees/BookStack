use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use bookstack_mcp::McpCtx;

use crate::extract::{resolve_auth_header, resolve_org};
use crate::state::AppState;

/// MCP Streamable HTTP transport endpoint (`POST /mcp`).
///
/// Stateless server: every request is authenticated independently via the
/// same `Authorization` schemes as the REST API, responses are plain JSON
/// (the spec-permitted alternative to an SSE stream), and no session id is
/// required. The acting org comes from the credential (API tokens and OAuth
/// tokens are org-bound) or an `X-Org-Id` header.
pub async fn handle_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let base_url = state.core.config.public_url.clone();
    let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok())
    else {
        return unauthorized(&base_url);
    };
    let resolved = match resolve_auth_header(&state.core, auth_header).await {
        Ok(resolved) => resolved,
        Err(_) => return unauthorized(&base_url),
    };
    let header_org = headers
        .get("x-org-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok());
    let (org_id, org_role) = match resolve_org(&state.core, &resolved, header_org).await {
        Ok(pair) => pair,
        Err(_) => return unauthorized(&base_url),
    };
    let ctx = McpCtx { user: resolved.user, org_id, org_role };

    let message: Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("parse error: {err}") },
                })),
            )
                .into_response();
        }
    };

    if message.is_array() {
        // JSON-RPC batching was removed in MCP 2025-06-18.
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32600, "message": "batch requests are not supported" },
            })),
        )
            .into_response();
    }

    match state.mcp.handle(&ctx, message).await {
        Some(response) => Json(response).into_response(),
        // Notification: acknowledged with no body.
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// GET /mcp — this server does not offer a server-initiated SSE stream.
pub async fn handle_get() -> Response {
    StatusCode::METHOD_NOT_ALLOWED.into_response()
}

/// DELETE /mcp — session termination is a no-op for this stateless server.
pub async fn handle_delete() -> Response {
    StatusCode::OK.into_response()
}

/// 401 with RFC 9728 resource-metadata pointer so OAuth-capable MCP clients
/// can discover the authorization server and start the flow.
fn unauthorized(base_url: &str) -> Response {
    let www = format!(
        "Bearer resource_metadata=\"{base_url}/.well-known/oauth-protected-resource\""
    );
    (
        StatusCode::UNAUTHORIZED,
        [("WWW-Authenticate", www.as_str())],
        Json(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": { "code": -32001, "message": "unauthorized: provide Authorization: Bearer <jwt> or Token <id>:<secret>, or complete the OAuth flow advertised in WWW-Authenticate" },
        })),
    )
        .into_response()
}
