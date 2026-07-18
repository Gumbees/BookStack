//! Built-in OAuth 2.1 authorization server for MCP clients (and any other
//! OAuth app), per the MCP authorization spec:
//!
//! - RFC 8414 authorization-server metadata + RFC 9728 protected-resource
//!   metadata for discovery
//! - RFC 7591 dynamic client registration (public clients)
//! - `GET /oauth/authorize` — hands off to the SPA consent page, which uses
//!   the normal BookStack session (password login or any configured SSO
//!   provider) and POSTs approval back
//! - `POST /oauth/token` — authorization_code with mandatory PKCE (S256) and
//!   rotating refresh_token grants
//!
//! Access tokens are org-bound BookStack JWTs, so they work on `/mcp` and
//! the REST API alike.

use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use chrono::Duration;
use serde::Deserialize;
use serde_json::{json, Value};

use bookstack_core::services::{oauth, orgs, users};
use bookstack_core::{auth, CoreError};

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthedUser;
use crate::state::AppState;

// ---- discovery ----

pub async fn authorization_server_metadata(State(state): State<AppState>) -> Json<Value> {
    let base = &state.core.config.public_url;
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "registration_endpoint": format!("{base}/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": ["bookstack"],
    }))
}

pub async fn protected_resource_metadata(State(state): State<AppState>) -> Json<Value> {
    let base = &state.core.config.public_url;
    Json(json!({
        "resource": format!("{base}/mcp"),
        "authorization_servers": [base],
        "bearer_methods_supported": ["header"],
        "resource_name": "BookStack MCP",
    }))
}

// ---- dynamic client registration (RFC 7591, public clients) ----

#[derive(Deserialize)]
pub struct RegisterRequest {
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
}

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> ApiResult<Response> {
    let name = body.client_name.unwrap_or_else(|| "MCP Client".to_string());
    let client = oauth::register_client(&state.core.db, &name, &body.redirect_uris).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "client_id": client.client_id,
            "client_name": client.name,
            "redirect_uris": client.redirect_uris,
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
        })),
    )
        .into_response())
}

// ---- authorize: validate then hand off to the SPA consent page ----

#[derive(Deserialize)]
pub struct AuthorizeQuery {
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub response_type: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub scope: Option<String>,
}

pub async fn authorize(
    State(state): State<AppState>,
    Query(query): Query<AuthorizeQuery>,
) -> Response {
    let Some(client_id) = query.client_id.as_deref() else {
        return bad_request("missing client_id");
    };
    let Some(redirect_uri) = query.redirect_uri.as_deref() else {
        return bad_request("missing redirect_uri");
    };
    let client = match oauth::get_client(&state.core.db, client_id).await {
        Ok(client) => client,
        Err(_) => return bad_request("unknown client_id"),
    };
    if !client.redirect_uri_allowed(redirect_uri) {
        return bad_request("redirect_uri is not registered for this client");
    }
    if query.response_type.as_deref() != Some("code") {
        return bad_request("response_type must be 'code'");
    }
    if query.code_challenge.as_deref().unwrap_or("").is_empty()
        || query.code_challenge_method.as_deref() != Some("S256")
    {
        return bad_request("PKCE (code_challenge with S256) is required");
    }

    // Hand off to the SPA: it authenticates the user (password or SSO) and
    // POSTs /api/oauth/approve.
    let target = format!(
        "{}/oauth/consent?client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256&scope={}",
        state.core.config.public_url,
        url(client_id),
        url(redirect_uri),
        url(query.state.as_deref().unwrap_or("")),
        url(query.code_challenge.as_deref().unwrap_or("")),
        url(query.scope.as_deref().unwrap_or("bookstack")),
    );
    Redirect::temporary(&target).into_response()
}

/// Client display info for the consent page.
pub async fn client_info(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(client_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let client = oauth::get_client(&state.core.db, &client_id).await?;
    Ok(Json(json!({
        "client_id": client.client_id,
        "name": client.name,
        "redirect_uris": client.redirect_uris,
    })))
}

#[derive(Deserialize)]
pub struct ApproveRequest {
    pub client_id: String,
    pub redirect_uri: String,
    #[serde(default)]
    pub state: String,
    pub code_challenge: String,
    #[serde(default = "default_s256")]
    pub code_challenge_method: String,
    #[serde(default)]
    pub scope: Option<String>,
    /// Org the token acts in; must be one of the approver's orgs.
    pub org_id: i64,
}

fn default_s256() -> String {
    "S256".to_string()
}

pub async fn approve(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<ApproveRequest>,
) -> ApiResult<Json<Value>> {
    let client = oauth::get_client(&state.core.db, &body.client_id).await?;
    if !client.redirect_uri_allowed(&body.redirect_uri) {
        return Err(ApiError(CoreError::validation("redirect_uri is not registered for this client")));
    }
    // Membership check: the token will act inside this org.
    orgs::role_in(&state.core.db, user.0.id, body.org_id, user.0.role.is_admin()).await?;

    let code = oauth::create_code(
        &state.core.db,
        &body.client_id,
        user.0.id,
        Some(body.org_id),
        &body.redirect_uri,
        &body.code_challenge,
        &body.code_challenge_method,
        body.scope.as_deref().unwrap_or("bookstack"),
    )
    .await?;

    let separator = if body.redirect_uri.contains('?') { '&' } else { '?' };
    let redirect_to = format!(
        "{}{}code={}&state={}",
        body.redirect_uri,
        separator,
        url(&code),
        url(&body.state)
    );
    Ok(Json(json!({ "redirect_to": redirect_to })))
}

// ---- token endpoint ----

#[derive(Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub redirect_uri: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub code_verifier: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

pub async fn token(State(state): State<AppState>, Form(body): Form<TokenRequest>) -> Response {
    match issue(&state, body).await {
        Ok(json_body) => Json(json_body).into_response(),
        Err(description) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_grant", "error_description": description })),
        )
            .into_response(),
    }
}

async fn issue(state: &AppState, body: TokenRequest) -> Result<Value, String> {
    let db = &state.core.db;
    let client_id = body.client_id.as_deref().ok_or("client_id is required")?;
    oauth::get_client(db, client_id).await.map_err(|_| "unknown client_id".to_string())?;

    let (user_id, org_id, scope, refresh_token) = match body.grant_type.as_str() {
        "authorization_code" => {
            let code = body.code.as_deref().ok_or("code is required")?;
            let redirect_uri = body.redirect_uri.as_deref().ok_or("redirect_uri is required")?;
            let verifier = body.code_verifier.as_deref().ok_or("code_verifier is required")?;
            let redeemed = oauth::redeem_code(db, client_id, code, redirect_uri, verifier)
                .await
                .map_err(error_text)?;
            let refresh =
                oauth::issue_refresh_token(db, client_id, redeemed.user_id, redeemed.org_id, &redeemed.scope)
                    .await
                    .map_err(error_text)?;
            (redeemed.user_id, redeemed.org_id, redeemed.scope, refresh)
        }
        "refresh_token" => {
            let presented = body.refresh_token.as_deref().ok_or("refresh_token is required")?;
            let rotated = oauth::rotate_refresh_token(db, client_id, presented)
                .await
                .map_err(error_text)?;
            (rotated.user_id, rotated.org_id, rotated.scope, rotated.token)
        }
        other => return Err(format!("unsupported grant_type: {other}")),
    };

    let user = users::get(db, user_id).await.map_err(error_text)?;
    let access_token = auth::issue_jwt_for_org(
        &state.core.config.jwt_secret,
        user.id,
        &user.name,
        &user.role,
        org_id,
        Duration::seconds(oauth::ACCESS_TOKEN_TTL_SECONDS),
    )
    .map_err(error_text)?;

    Ok(json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": oauth::ACCESS_TOKEN_TTL_SECONDS,
        "refresh_token": refresh_token,
        "scope": scope,
    }))
}

fn error_text(err: CoreError) -> String {
    match err {
        CoreError::Validation(message) => message,
        CoreError::NotFound => "not found".to_string(),
        other => format!("{other}"),
    }
}

fn bad_request(message: &str) -> Response {
    (StatusCode::BAD_REQUEST, message.to_string()).into_response()
}

fn url(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}
