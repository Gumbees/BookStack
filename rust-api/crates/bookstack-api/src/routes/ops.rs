//! Operational endpoints: BookStack imports, encrypted backups, WAL-ship
//! status. Policy: global/sql backups + WAL status are system-admin; org
//! backups and imports require admin of the target org; the filesystem
//! target is restricted to global-scope backups.

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use bookstack_core::services::orgs;
use bookstack_core::CoreError;
use bookstack_ops::{backup, import, Target};

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

// ---- imports ----

#[derive(Deserialize)]
pub struct ImportRequest {
    pub base_url: String,
    pub token_id: String,
    pub token_secret: String,
    pub org_id: i64,
    #[serde(default)]
    pub rate_limit_per_minute: Option<u32>,
}

pub async fn start_import(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<ImportRequest>,
) -> ApiResult<Json<Value>> {
    require_org_admin(&state, &user, body.org_id).await?;
    if !body.base_url.starts_with("http://") && !body.base_url.starts_with("https://") {
        return Err(ApiError(CoreError::validation("base_url must be an http(s) URL")));
    }
    let (job_id,): (i64,) = sqlx::query_as(
        "INSERT INTO import_jobs (org_id, source_url, created_by) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(body.org_id)
    .bind(&body.base_url)
    .bind(user.0.id)
    .fetch_one(&state.core.db)
    .await
    .map_err(CoreError::from)?;

    let config = import::ImportConfig {
        base_url: body.base_url,
        token_id: body.token_id,
        token_secret: body.token_secret,
        org_id: body.org_id,
        user_id: user.0.id,
        rate_limit_per_minute: body.rate_limit_per_minute.unwrap_or(90),
    };
    tokio::spawn(import::run(state.core.db.clone(), job_id, config));
    Ok(Json(json!({ "job_id": job_id, "status": "running" })))
}

pub async fn get_import(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let row: Option<(i64, i64, String, String, Value)> = sqlx::query_as(
        "SELECT id, org_id, source_url, status, progress FROM import_jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.core.db)
    .await
    .map_err(CoreError::from)?;
    let (id, org_id, source_url, status, progress) = row.ok_or(ApiError(CoreError::NotFound))?;
    require_org_admin(&state, &user, org_id).await?;
    Ok(Json(json!({
        "id": id, "org_id": org_id, "source_url": source_url,
        "status": status, "progress": progress,
    })))
}

pub async fn list_imports(State(state): State<AppState>, user: AuthedUser) -> ApiResult<Json<Value>> {
    let rows: Vec<(i64, i64, String, String, Value)> = sqlx::query_as(
        "SELECT id, org_id, source_url, status, progress FROM import_jobs ORDER BY id DESC LIMIT 50",
    )
    .fetch_all(&state.core.db)
    .await
    .map_err(CoreError::from)?;
    let mut visible = Vec::new();
    for (id, org_id, source_url, status, progress) in rows {
        if require_org_admin(&state, &user, org_id).await.is_ok() {
            visible.push(json!({
                "id": id, "org_id": org_id, "source_url": source_url,
                "status": status, "progress": progress,
            }));
        }
    }
    Ok(Json(json!({ "data": visible })))
}

// ---- backups ----

#[derive(Deserialize)]
pub struct BackupRequest {
    pub scope: String,
    #[serde(default)]
    pub org_id: Option<i64>,
    pub target: String,
}

pub async fn start_backup(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<BackupRequest>,
) -> ApiResult<Json<Value>> {
    let target = Target::parse(&body.target)
        .map_err(|e| ApiError(CoreError::validation(e.to_string())))?;

    let org_id = match body.scope.as_str() {
        "org" => {
            let org = body
                .org_id
                .ok_or(ApiError(CoreError::validation("org_id is required for org backups")))?;
            require_org_admin(&state, &user, org).await?;
            if target == Target::Filesystem {
                return Err(ApiError(CoreError::validation(
                    "the filesystem target is restricted to global backups; per-org backups must go to object storage",
                )));
            }
            Some(org)
        }
        "global" | "sql" => {
            user.require_system_admin()?;
            None
        }
        other => {
            return Err(ApiError(CoreError::validation(format!(
                "unknown scope '{other}' (expected global, org or sql)"
            ))))
        }
    };

    let (backup_id,): (i64,) = sqlx::query_as(
        "INSERT INTO backups (scope, org_id, target, location, created_by) VALUES ($1, $2, $3, '', $4) RETURNING id",
    )
    .bind(&body.scope)
    .bind(org_id)
    .bind(target.as_str())
    .bind(user.0.id)
    .fetch_one(&state.core.db)
    .await
    .map_err(CoreError::from)?;

    let db = state.core.db.clone();
    let database_url = state.core.config.database_url.clone();
    let scope = body.scope.clone();
    tokio::spawn(async move {
        let outcome = backup::run(&db, &database_url, &scope, org_id, target).await;
        let _ = match outcome {
            Ok(done) => {
                sqlx::query(
                    "UPDATE backups SET status = 'completed', location = $2, size_bytes = $3, updated_at = now() WHERE id = $1",
                )
                .bind(backup_id)
                .bind(format!("{}|{}", done.location, done.object_name))
                .bind(done.size_bytes)
                .execute(&db)
                .await
            }
            Err(err) => {
                sqlx::query(
                    "UPDATE backups SET status = 'failed', error = $2, updated_at = now() WHERE id = $1",
                )
                .bind(backup_id)
                .bind(err.to_string())
                .execute(&db)
                .await
            }
        };
    });
    Ok(Json(json!({ "backup_id": backup_id, "status": "running" })))
}

pub async fn list_backups(State(state): State<AppState>, user: AuthedUser) -> ApiResult<Json<Value>> {
    let rows: Vec<(i64, String, Option<i64>, String, String, i64, String, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, scope, org_id, target, location, size_bytes, status, error, created_at
             FROM backups ORDER BY id DESC LIMIT 100",
        )
        .fetch_all(&state.core.db)
        .await
        .map_err(CoreError::from)?;
    let mut visible = Vec::new();
    for (id, scope, org_id, target, location, size, status, error, created_at) in rows {
        let allowed = match org_id {
            Some(org) => require_org_admin(&state, &user, org).await.is_ok(),
            None => user.0.role.is_admin(),
        };
        if allowed {
            visible.push(json!({
                "id": id, "scope": scope, "org_id": org_id, "target": target,
                "location": location.split('|').next().unwrap_or(&location),
                "size_bytes": size, "status": status, "error": error, "created_at": created_at,
                "encrypted": true,
            }));
        }
    }
    Ok(Json(json!({ "data": visible })))
}

/// Download + decrypt + parse a stored backup to prove it is restorable.
pub async fn verify_backup(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let row: Option<(String, Option<i64>, String, String, String)> = sqlx::query_as(
        "SELECT scope, org_id, target, location, status FROM backups WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.core.db)
    .await
    .map_err(CoreError::from)?;
    let (scope, org_id, target, location, status) = row.ok_or(ApiError(CoreError::NotFound))?;
    match org_id {
        Some(org) => require_org_admin(&state, &user, org).await?,
        None => user.require_system_admin()?,
    }
    if status != "completed" {
        return Err(ApiError(CoreError::validation("backup is not in completed state")));
    }
    let object_name = location.split('|').nth(1).ok_or(ApiError(CoreError::NotFound))?.to_string();
    let target = Target::parse(&target).map_err(|e| ApiError(CoreError::validation(e.to_string())))?;
    let report = backup::verify(target, &object_name, &scope)
        .await
        .map_err(|e| ApiError(CoreError::validation(e.to_string())))?;
    Ok(Json(report))
}

// ---- WAL shipping ----

pub async fn walship_status(State(state): State<AppState>, user: AuthedUser) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    match &state.walship {
        Some(shipper) => Ok(Json(shipper.status())),
        None => Ok(Json(json!({ "enabled": false, "note": "set WALSHIP_ENABLED=true (plus WALSHIP_TARGET and BACKUP_ENCRYPTION_KEY) to stream WAL segments" }))),
    }
}
