//! Admin settings: global auth configuration (system admins) and per-org
//! overrides (org admins). Org-level provider management is gated by the
//! global `auth.orgs_can_manage` checkbox, inherited unless the org has an
//! explicit override; system admins always pass.

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use bookstack_core::services::{auth_providers, orgs, settings};
use bookstack_core::CoreError;

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthedUser;
use crate::state::AppState;

// ---- global (system admin) ----

pub async fn get_global_settings(
    State(state): State<AppState>,
    user: AuthedUser,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    let orgs_can_manage =
        settings::global_bool(&state.core.db, settings::ORGS_CAN_MANAGE_AUTH, false).await?;
    Ok(Json(json!({ "orgs_can_manage_auth": orgs_can_manage })))
}

#[derive(Deserialize)]
pub struct GlobalSettingsRequest {
    pub orgs_can_manage_auth: Option<bool>,
}

pub async fn update_global_settings(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<GlobalSettingsRequest>,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    if let Some(allowed) = body.orgs_can_manage_auth {
        settings::set(&state.core.db, None, settings::ORGS_CAN_MANAGE_AUTH, &json!(allowed)).await?;
    }
    get_global_settings(State(state), user).await
}

pub async fn list_global_providers(
    State(state): State<AppState>,
    user: AuthedUser,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    let providers = auth_providers::list_scope(&state.core.db, None).await?;
    Ok(Json(json!({ "data": providers })))
}

pub async fn create_global_provider(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<auth_providers::ProviderInput>,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    let provider = auth_providers::create(&state.core.db, None, &body).await?;
    Ok(Json(serde_json::to_value(provider).unwrap()))
}

pub async fn update_global_provider(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
    Json(body): Json<auth_providers::ProviderInput>,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    let provider = auth_providers::update(&state.core.db, None, id, &body).await?;
    Ok(Json(serde_json::to_value(provider).unwrap()))
}

pub async fn delete_global_provider(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    auth_providers::delete(&state.core.db, None, id).await?;
    Ok(Json(json!({ "deleted": true })))
}

// ---- per-org (org admin) ----

async fn require_org_admin(state: &AppState, user: &AuthedUser, org_id: i64) -> Result<(), ApiError> {
    let role = orgs::role_in(&state.core.db, user.0.id, org_id, user.0.role.is_admin()).await?;
    if role.is_admin() {
        Ok(())
    } else {
        Err(ApiError(CoreError::Forbidden))
    }
}

/// May this org manage its own providers? System admins always may; org
/// admins only when the global checkbox (or an org-level override set by a
/// system admin) allows it.
async fn org_can_manage(state: &AppState, user: &AuthedUser, org_id: i64) -> ApiResult<bool> {
    if user.0.role.is_admin() {
        return Ok(true);
    }
    Ok(settings::effective_bool(&state.core.db, org_id, settings::ORGS_CAN_MANAGE_AUTH, false).await?)
}

pub async fn get_org_settings(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    let inherit =
        settings::effective_bool(&state.core.db, org_id, settings::INHERIT_GLOBAL_AUTH, true).await?;
    let can_manage = org_can_manage(&state, &user, org_id).await?;
    Ok(Json(json!({
        "inherit_global_auth": inherit,
        "can_manage_providers": can_manage,
    })))
}

#[derive(Deserialize)]
pub struct OrgSettingsRequest {
    pub inherit_global_auth: Option<bool>,
}

pub async fn update_org_settings(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
    Json(body): Json<OrgSettingsRequest>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    if let Some(inherit) = body.inherit_global_auth {
        settings::set(&state.core.db, Some(org_id), settings::INHERIT_GLOBAL_AUTH, &json!(inherit))
            .await?;
    }
    get_org_settings(State(state), user, Path(org_id)).await
}

pub async fn list_org_providers(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    let providers = auth_providers::list_scope(&state.core.db, Some(org_id)).await?;
    Ok(Json(json!({ "data": providers })))
}

pub async fn create_org_provider(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
    Json(body): Json<auth_providers::ProviderInput>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    if !org_can_manage(&state, &user, org_id).await? {
        return Err(ApiError(CoreError::validation(
            "org-level auth providers are disabled by the instance administrator",
        )));
    }
    let provider = auth_providers::create(&state.core.db, Some(org_id), &body).await?;
    Ok(Json(serde_json::to_value(provider).unwrap()))
}

pub async fn update_org_provider(
    State(state): State<AppState>,
    user: AuthedUser,
    Path((org_id, id)): Path<(i64, i64)>,
    Json(body): Json<auth_providers::ProviderInput>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    if !org_can_manage(&state, &user, org_id).await? {
        return Err(ApiError(CoreError::validation(
            "org-level auth providers are disabled by the instance administrator",
        )));
    }
    let provider = auth_providers::update(&state.core.db, Some(org_id), id, &body).await?;
    Ok(Json(serde_json::to_value(provider).unwrap()))
}

pub async fn delete_org_provider(
    State(state): State<AppState>,
    user: AuthedUser,
    Path((org_id, id)): Path<(i64, i64)>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, org_id).await?;
    if !org_can_manage(&state, &user, org_id).await? {
        return Err(ApiError(CoreError::validation(
            "org-level auth providers are disabled by the instance administrator",
        )));
    }
    auth_providers::delete(&state.core.db, Some(org_id), id).await?;
    Ok(Json(json!({ "deleted": true })))
}
