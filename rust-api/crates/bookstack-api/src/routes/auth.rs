use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use bookstack_core::auth;

use crate::error::ApiResult;
use crate::extract::AuthedUser;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> ApiResult<Json<Value>> {
    let (token, user) = auth::login(
        &state.core.db,
        &state.core.config.jwt_secret,
        &body.email,
        &body.password,
    )
    .await?;
    Ok(Json(json!({ "token": token, "user": user })))
}

pub async fn me(user: AuthedUser) -> Json<Value> {
    Json(json!({ "user": user.0 }))
}

#[derive(Deserialize)]
pub struct CreateTokenRequest {
    pub name: String,
}

pub async fn create_token(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<CreateTokenRequest>,
) -> ApiResult<Json<Value>> {
    let created = auth::create_api_token(&state.core.db, user.0.id, &body.name).await?;
    Ok(Json(json!({
        "token": created,
        "note": "Store the secret now; it is not shown again. Use header: Authorization: Token <token_id>:<secret>",
    })))
}

pub async fn list_tokens(State(state): State<AppState>, user: AuthedUser) -> ApiResult<Json<Value>> {
    let tokens = auth::list_api_tokens(&state.core.db, user.0.id).await?;
    Ok(Json(json!({ "data": tokens })))
}

pub async fn delete_token(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    auth::delete_api_token(&state.core.db, user.0.id, id).await?;
    Ok(Json(json!({ "deleted": true })))
}
