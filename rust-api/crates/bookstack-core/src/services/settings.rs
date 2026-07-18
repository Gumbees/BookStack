//! Key/value settings with global → org inheritance: an org-scoped value
//! overrides the global one; absent both, the coded default applies.

use serde_json::Value;
use sqlx::PgPool;

use crate::Result;

/// Global checkbox: may org admins add/edit their own auth providers?
pub const ORGS_CAN_MANAGE_AUTH: &str = "auth.orgs_can_manage";
/// Per-org: inherit global auth providers (default true).
pub const INHERIT_GLOBAL_AUTH: &str = "auth.inherit_global";

pub async fn get_raw(db: &PgPool, org_id: Option<i64>, key: &str) -> Result<Option<Value>> {
    let row: Option<(Value,)> = match org_id {
        Some(org) => {
            sqlx::query_as("SELECT value FROM settings WHERE org_id = $1 AND key = $2")
                .bind(org)
                .bind(key)
                .fetch_optional(db)
                .await?
        }
        None => {
            sqlx::query_as("SELECT value FROM settings WHERE org_id IS NULL AND key = $1")
                .bind(key)
                .fetch_optional(db)
                .await?
        }
    };
    Ok(row.map(|(value,)| value))
}

/// Effective value for an org: org override, else global, else None.
pub async fn effective(db: &PgPool, org_id: i64, key: &str) -> Result<Option<Value>> {
    if let Some(value) = get_raw(db, Some(org_id), key).await? {
        return Ok(Some(value));
    }
    get_raw(db, None, key).await
}

pub async fn effective_bool(db: &PgPool, org_id: i64, key: &str, default: bool) -> Result<bool> {
    Ok(effective(db, org_id, key)
        .await?
        .and_then(|value| value.as_bool())
        .unwrap_or(default))
}

pub async fn global_bool(db: &PgPool, key: &str, default: bool) -> Result<bool> {
    Ok(get_raw(db, None, key)
        .await?
        .and_then(|value| value.as_bool())
        .unwrap_or(default))
}

pub async fn set(db: &PgPool, org_id: Option<i64>, key: &str, value: &Value) -> Result<()> {
    sqlx::query(
        "INSERT INTO settings (org_id, key, value) VALUES ($1, $2, $3)
         ON CONFLICT ((coalesce(org_id, 0)), key) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
    )
    .bind(org_id)
    .bind(key)
    .bind(value)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn unset(db: &PgPool, org_id: Option<i64>, key: &str) -> Result<()> {
    match org_id {
        Some(org) => {
            sqlx::query("DELETE FROM settings WHERE org_id = $1 AND key = $2")
                .bind(org)
                .bind(key)
                .execute(db)
                .await?;
        }
        None => {
            sqlx::query("DELETE FROM settings WHERE org_id IS NULL AND key = $1")
                .bind(key)
                .execute(db)
                .await?;
        }
    }
    Ok(())
}
