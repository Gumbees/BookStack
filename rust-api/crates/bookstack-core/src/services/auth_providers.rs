//! External OAuth2/OIDC login providers ("Sign in with …").
//!
//! Providers exist at two scopes: global (org_id NULL, managed by system
//! admins) and per-org (managed by org admins when the global
//! `auth.orgs_can_manage` checkbox permits). An org's effective provider set
//! is the global set (unless the org disables `auth.inherit_global`) plus
//! its own.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::settings;
use crate::{CoreError, Result};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct AuthProvider {
    pub id: i64,
    pub org_id: Option<i64>,
    pub name: String,
    pub client_id: String,
    #[serde(skip)]
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub scopes: String,
    pub enabled: bool,
    pub auto_register: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Provider info safe for the (pre-auth) login page.
#[derive(Debug, Serialize)]
pub struct PublicProvider {
    pub id: i64,
    pub name: String,
    pub org_id: Option<i64>,
}

pub async fn get(db: &PgPool, id: i64) -> Result<AuthProvider> {
    sqlx::query_as::<_, AuthProvider>("SELECT * FROM auth_providers WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::NotFound)
}

/// Providers at one scope (global when `org_id` is None).
pub async fn list_scope(db: &PgPool, org_id: Option<i64>) -> Result<Vec<AuthProvider>> {
    let rows = match org_id {
        Some(org) => {
            sqlx::query_as::<_, AuthProvider>(
                "SELECT * FROM auth_providers WHERE org_id = $1 ORDER BY id",
            )
            .bind(org)
            .fetch_all(db)
            .await?
        }
        None => {
            sqlx::query_as::<_, AuthProvider>(
                "SELECT * FROM auth_providers WHERE org_id IS NULL ORDER BY id",
            )
            .fetch_all(db)
            .await?
        }
    };
    Ok(rows)
}

/// The effective, enabled provider set for a login surface. With no org
/// context: global providers only. With an org: global (if inherited) + own.
pub async fn effective_public(db: &PgPool, org_id: Option<i64>) -> Result<Vec<PublicProvider>> {
    let mut providers: Vec<AuthProvider> = Vec::new();
    match org_id {
        None => providers.extend(list_scope(db, None).await?),
        Some(org) => {
            if settings::effective_bool(db, org, settings::INHERIT_GLOBAL_AUTH, true).await? {
                providers.extend(list_scope(db, None).await?);
            }
            providers.extend(list_scope(db, Some(org)).await?);
        }
    }
    Ok(providers
        .into_iter()
        .filter(|p| p.enabled)
        .map(|p| PublicProvider { id: p.id, name: p.name, org_id: p.org_id })
        .collect())
}

#[derive(Debug, Deserialize)]
pub struct ProviderInput {
    pub name: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    #[serde(default)]
    pub scopes: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub auto_register: Option<bool>,
}

fn validate(input: &ProviderInput) -> Result<()> {
    for (field, value) in [
        ("authorize_url", &input.authorize_url),
        ("token_url", &input.token_url),
        ("userinfo_url", &input.userinfo_url),
    ] {
        if !value.starts_with("https://") && !value.starts_with("http://") {
            return Err(CoreError::validation(format!("{field} must be an http(s) URL")));
        }
    }
    if input.name.trim().is_empty() {
        return Err(CoreError::validation("name is required"));
    }
    if input.client_id.trim().is_empty() {
        return Err(CoreError::validation("client_id is required"));
    }
    Ok(())
}

pub async fn create(db: &PgPool, org_id: Option<i64>, input: &ProviderInput) -> Result<AuthProvider> {
    validate(input)?;
    Ok(sqlx::query_as::<_, AuthProvider>(
        "INSERT INTO auth_providers (org_id, name, client_id, client_secret, authorize_url, token_url, userinfo_url, scopes, enabled, auto_register)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING *",
    )
    .bind(org_id)
    .bind(input.name.trim())
    .bind(input.client_id.trim())
    .bind(&input.client_secret)
    .bind(&input.authorize_url)
    .bind(&input.token_url)
    .bind(&input.userinfo_url)
    .bind(input.scopes.clone().unwrap_or_else(|| "openid profile email".to_string()))
    .bind(input.enabled.unwrap_or(true))
    .bind(input.auto_register.unwrap_or(true))
    .fetch_one(db)
    .await?)
}

/// Update a provider, verifying it belongs to the expected scope.
/// An empty client_secret keeps the stored one.
pub async fn update(
    db: &PgPool,
    org_id: Option<i64>,
    id: i64,
    input: &ProviderInput,
) -> Result<AuthProvider> {
    validate(input)?;
    let current = get(db, id).await?;
    if current.org_id != org_id {
        return Err(CoreError::NotFound);
    }
    let secret = if input.client_secret.is_empty() {
        current.client_secret
    } else {
        input.client_secret.clone()
    };
    Ok(sqlx::query_as::<_, AuthProvider>(
        "UPDATE auth_providers SET name = $2, client_id = $3, client_secret = $4, authorize_url = $5,
                token_url = $6, userinfo_url = $7, scopes = $8, enabled = $9, auto_register = $10, updated_at = now()
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(input.name.trim())
    .bind(input.client_id.trim())
    .bind(secret)
    .bind(&input.authorize_url)
    .bind(&input.token_url)
    .bind(&input.userinfo_url)
    .bind(input.scopes.clone().unwrap_or_else(|| "openid profile email".to_string()))
    .bind(input.enabled.unwrap_or(true))
    .bind(input.auto_register.unwrap_or(true))
    .fetch_one(db)
    .await?)
}

pub async fn delete(db: &PgPool, org_id: Option<i64>, id: i64) -> Result<()> {
    let current = get(db, id).await?;
    if current.org_id != org_id {
        return Err(CoreError::NotFound);
    }
    sqlx::query("DELETE FROM auth_providers WHERE id = $1").bind(id).execute(db).await?;
    Ok(())
}
