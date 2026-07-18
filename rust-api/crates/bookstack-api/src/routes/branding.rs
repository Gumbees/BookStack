//! Branding endpoints. Reads are public (colors/logos render on the login
//! page before auth); writes are org-admin (org scope) or system-admin
//! (global scope).

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use bookstack_core::services::{branding, orgs};
use bookstack_core::CoreError;

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthedUser;
use crate::state::AppState;

async fn require_org_admin(state: &AppState, user: &AuthedUser, org_id: i64) -> Result<(), ApiError> {
    let role = orgs::role_in(&state.core.db, user.0.id, org_id, user.0.role.is_admin()).await?;
    if role.is_admin() {
        Ok(())
    } else {
        Err(ApiError(CoreError::Forbidden))
    }
}

#[derive(Deserialize)]
pub struct BrandingQuery {
    pub org_id: Option<i64>,
}

/// Effective branding (built-in ← global ← org). Public.
pub async fn effective(
    State(state): State<AppState>,
    Query(query): Query<BrandingQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(branding::effective(&state.core.db, query.org_id).await?))
}

/// Logo bytes for a surface (org logo falling back to global). Public.
pub async fn logo(
    State(state): State<AppState>,
    Query(query): Query<BrandingQuery>,
) -> Response {
    match branding::get_logo(&state.core.db, query.org_id).await {
        Ok((mime, data)) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime),
                (header::CACHE_CONTROL, "public, max-age=300".to_string()),
            ],
            data,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

// ---- org scope (org admins) ----

pub async fn get_org_branding(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    Ok(Json(branding::get_scope(&state.core.db, Some(org_id)).await?))
}

pub async fn set_org_branding(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
    Json(body): Json<branding::BrandingInput>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    Ok(Json(branding::set_scope(&state.core.db, Some(org_id), &body).await?))
}

pub async fn upload_org_logo(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    let mime = content_type(&headers)?;
    branding::set_logo(&state.core.db, Some(org_id), &mime, &body).await?;
    Ok(Json(json!({ "uploaded": true })))
}

pub async fn delete_org_logo(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    branding::delete_logo(&state.core.db, Some(org_id)).await?;
    Ok(Json(json!({ "deleted": true })))
}

// ---- global scope (system admins) ----

pub async fn get_global_branding(
    State(state): State<AppState>,
    user: AuthedUser,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    Ok(Json(branding::get_scope(&state.core.db, None).await?))
}

pub async fn set_global_branding(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<branding::BrandingInput>,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    Ok(Json(branding::set_scope(&state.core.db, None, &body).await?))
}

pub async fn upload_global_logo(
    State(state): State<AppState>,
    user: AuthedUser,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    let mime = content_type(&headers)?;
    branding::set_logo(&state.core.db, None, &mime, &body).await?;
    Ok(Json(json!({ "uploaded": true })))
}

pub async fn delete_global_logo(
    State(state): State<AppState>,
    user: AuthedUser,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    branding::delete_logo(&state.core.db, None).await?;
    Ok(Json(json!({ "deleted": true })))
}

fn content_type(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(';').next().unwrap_or(v).trim().to_string())
        .ok_or(ApiError(CoreError::validation("Content-Type header is required")))
}
