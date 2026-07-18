use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use bookstack_core::models::Role;
use bookstack_core::services::orgs;
use bookstack_core::CoreError;

use crate::error::{ApiError, ApiResult};
use crate::extract::{AuthedUser, OrgCtx};
use crate::state::AppState;

/// The caller's org memberships (drives the org switcher).
pub async fn my_orgs(State(state): State<AppState>, user: AuthedUser) -> ApiResult<Json<Value>> {
    let memberships = orgs::memberships(&state.core.db, user.0.id).await?;
    Ok(Json(json!({ "data": memberships })))
}

/// Any authenticated user may create an org; they become its admin.
pub async fn create_org(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<orgs::CreateOrg>,
) -> ApiResult<Json<Value>> {
    let org = orgs::create(&state.core.db, user.0.id, &body).await?;
    Ok(Json(serde_json::to_value(org).unwrap()))
}

async fn require_org_admin_of(
    state: &AppState,
    user: &AuthedUser,
    org_id: i64,
) -> Result<(), ApiError> {
    let role = orgs::role_in(&state.core.db, user.0.id, org_id, user.0.role.is_admin()).await?;
    if role.is_admin() {
        Ok(())
    } else {
        Err(ApiError(CoreError::Forbidden))
    }
}

pub async fn members(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_org_admin_of(&state, &user, org_id).await?;
    Ok(Json(json!({ "data": orgs::members(&state.core.db, org_id).await? })))
}

#[derive(Deserialize)]
pub struct AddMemberRequest {
    pub email: String,
    #[serde(default)]
    pub role: Option<String>,
}

pub async fn add_member(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(org_id): Path<i64>,
    Json(body): Json<AddMemberRequest>,
) -> ApiResult<Json<Value>> {
    require_org_admin_of(&state, &user, org_id).await?;
    let role = match body.role.as_deref() {
        Some(r) => Role::parse(r)
            .ok_or_else(|| ApiError(CoreError::validation("role must be admin, editor or viewer")))?,
        None => Role::Viewer,
    };
    let member = orgs::upsert_member(&state.core.db, org_id, &body.email, role).await?;
    Ok(Json(serde_json::to_value(member).unwrap()))
}

pub async fn remove_member(
    State(state): State<AppState>,
    user: AuthedUser,
    Path((org_id, member_id)): Path<(i64, i64)>,
) -> ApiResult<Json<Value>> {
    require_org_admin_of(&state, &user, org_id).await?;
    orgs::remove_member(&state.core.db, org_id, member_id).await?;
    Ok(Json(json!({ "removed": true })))
}

/// Details of the active org (name/slug/role) for the header widget.
pub async fn current(State(state): State<AppState>, ctx: OrgCtx) -> ApiResult<Json<Value>> {
    let org = orgs::get(&state.core.db, ctx.org_id).await?;
    Ok(Json(json!({ "org": org, "role": ctx.org_role })))
}
