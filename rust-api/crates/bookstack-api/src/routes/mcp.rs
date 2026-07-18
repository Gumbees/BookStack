use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::extract::resolve_auth_header;
use crate::state::AppState;

/// MCP Streamable HTTP transport endpoint (`POST /mcp`).
///
/// Stateless server: every request is authenticated independently via the
/// same `Authorization` schemes as the REST API, responses are plain JSON
/// (the spec-permitted alternative to an SSE stream), and no session id is
/// required.
pub async fn handle_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok())
    else {
        return unauthorized();
    };
    let user = match resolve_auth_header(&state.core, auth_header).await {
        Ok(user) => user,
        Err(_) => return unauthorized(),
    };

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

    match state.mcp.handle(&user, message).await {
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

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [("WWW-Authenticate", "Bearer, Token")],
        Json(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": { "code": -32001, "message": "unauthorized: provide Authorization: Bearer <jwt> or Token <id>:<secret>" },
        })),
    )
        .into_response()
}
