//! Storage + validation for the built-in MCP OAuth authorization server:
//! dynamically-registered clients, single-use PKCE authorization codes, and
//! rotating refresh tokens.

use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};

use crate::{CoreError, Result};

pub const CODE_TTL_MINUTES: i64 = 10;
pub const ACCESS_TOKEN_TTL_SECONDS: i64 = 3600;
pub const REFRESH_TOKEN_TTL_DAYS: i64 = 30;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct OAuthClient {
    pub client_id: String,
    #[serde(skip)]
    pub client_secret: Option<String>,
    pub name: String,
    pub redirect_uris: Value,
    pub created_at: DateTime<Utc>,
}

impl OAuthClient {
    pub fn redirect_uri_allowed(&self, uri: &str) -> bool {
        self.redirect_uris
            .as_array()
            .map(|uris| uris.iter().any(|u| u.as_str() == Some(uri)))
            .unwrap_or(false)
    }
}

fn random_token(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// RFC 7636 S256: base64url(sha256(verifier)) without padding.
pub fn pkce_s256(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

/// Register a public client (RFC 7591 subset).
pub async fn register_client(
    db: &PgPool,
    name: &str,
    redirect_uris: &[String],
) -> Result<OAuthClient> {
    if redirect_uris.is_empty() {
        return Err(CoreError::validation("redirect_uris is required"));
    }
    for uri in redirect_uris {
        let ok = uri.starts_with("https://")
            || uri.starts_with("http://localhost")
            || uri.starts_with("http://127.0.0.1");
        if !ok {
            return Err(CoreError::validation(
                "redirect_uris must be https:// (or http://localhost for development)",
            ));
        }
    }
    let client_id = format!("mcp-{}", random_token(24));
    Ok(sqlx::query_as::<_, OAuthClient>(
        "INSERT INTO oauth_clients (client_id, client_secret, name, redirect_uris)
         VALUES ($1, NULL, $2, $3) RETURNING *",
    )
    .bind(&client_id)
    .bind(name)
    .bind(serde_json::json!(redirect_uris))
    .fetch_one(db)
    .await?)
}

pub async fn get_client(db: &PgPool, client_id: &str) -> Result<OAuthClient> {
    sqlx::query_as::<_, OAuthClient>("SELECT * FROM oauth_clients WHERE client_id = $1")
        .bind(client_id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::NotFound)
}

/// Issue a single-use authorization code after user consent.
pub async fn create_code(
    db: &PgPool,
    client_id: &str,
    user_id: i64,
    org_id: Option<i64>,
    redirect_uri: &str,
    code_challenge: &str,
    code_challenge_method: &str,
    scope: &str,
) -> Result<String> {
    if code_challenge.is_empty() || code_challenge_method != "S256" {
        return Err(CoreError::validation("PKCE with S256 code_challenge is required"));
    }
    let code = random_token(48);
    sqlx::query(
        "INSERT INTO oauth_codes (code, client_id, user_id, org_id, redirect_uri, code_challenge, code_challenge_method, scope, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(&code)
    .bind(client_id)
    .bind(user_id)
    .bind(org_id)
    .bind(redirect_uri)
    .bind(code_challenge)
    .bind(code_challenge_method)
    .bind(scope)
    .bind(Utc::now() + Duration::minutes(CODE_TTL_MINUTES))
    .execute(db)
    .await?;
    Ok(code)
}

pub struct RedeemedCode {
    pub user_id: i64,
    pub org_id: Option<i64>,
    pub scope: String,
}

/// Redeem an authorization code (single use): verifies client, redirect_uri
/// and the PKCE verifier, then deletes the code.
pub async fn redeem_code(
    db: &PgPool,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<RedeemedCode> {
    #[derive(FromRow)]
    struct CodeRow {
        client_id: String,
        user_id: i64,
        org_id: Option<i64>,
        redirect_uri: String,
        code_challenge: String,
        scope: String,
        expires_at: DateTime<Utc>,
    }
    // Delete-and-return makes redemption atomic and single-use.
    let row: Option<CodeRow> = sqlx::query_as(
        "DELETE FROM oauth_codes WHERE code = $1
         RETURNING client_id, user_id, org_id, redirect_uri, code_challenge, scope, expires_at",
    )
    .bind(code)
    .fetch_optional(db)
    .await?;
    let row = row.ok_or_else(|| CoreError::validation("invalid_grant: unknown or used code"))?;
    if row.client_id != client_id {
        return Err(CoreError::validation("invalid_grant: client mismatch"));
    }
    if row.redirect_uri != redirect_uri {
        return Err(CoreError::validation("invalid_grant: redirect_uri mismatch"));
    }
    if row.expires_at < Utc::now() {
        return Err(CoreError::validation("invalid_grant: code expired"));
    }
    if pkce_s256(code_verifier) != row.code_challenge {
        return Err(CoreError::validation("invalid_grant: PKCE verification failed"));
    }
    Ok(RedeemedCode { user_id: row.user_id, org_id: row.org_id, scope: row.scope })
}

pub struct IssuedRefresh {
    pub token: String,
    pub user_id: i64,
    pub org_id: Option<i64>,
    pub scope: String,
}

/// Mint a refresh token (stored hashed).
pub async fn issue_refresh_token(
    db: &PgPool,
    client_id: &str,
    user_id: i64,
    org_id: Option<i64>,
    scope: &str,
) -> Result<String> {
    let token = format!("rt-{}", random_token(48));
    sqlx::query(
        "INSERT INTO oauth_refresh_tokens (token_hash, client_id, user_id, org_id, scope, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(sha256_hex(&token))
    .bind(client_id)
    .bind(user_id)
    .bind(org_id)
    .bind(scope)
    .bind(Utc::now() + Duration::days(REFRESH_TOKEN_TTL_DAYS))
    .execute(db)
    .await?;
    Ok(token)
}

/// Rotate a refresh token: revoke the presented one and mint a successor.
pub async fn rotate_refresh_token(
    db: &PgPool,
    client_id: &str,
    presented: &str,
) -> Result<IssuedRefresh> {
    #[derive(FromRow)]
    struct RefreshRow {
        client_id: String,
        user_id: i64,
        org_id: Option<i64>,
        scope: String,
        expires_at: DateTime<Utc>,
    }
    let row: Option<RefreshRow> = sqlx::query_as(
        "UPDATE oauth_refresh_tokens SET revoked_at = now()
         WHERE token_hash = $1 AND revoked_at IS NULL
         RETURNING client_id, user_id, org_id, scope, expires_at",
    )
    .bind(sha256_hex(presented))
    .fetch_optional(db)
    .await?;
    let row = row.ok_or_else(|| CoreError::validation("invalid_grant: unknown or revoked refresh token"))?;
    if row.client_id != client_id {
        return Err(CoreError::validation("invalid_grant: client mismatch"));
    }
    if row.expires_at < Utc::now() {
        return Err(CoreError::validation("invalid_grant: refresh token expired"));
    }
    let token = issue_refresh_token(db, client_id, row.user_id, row.org_id, &row.scope).await?;
    Ok(IssuedRefresh { token, user_id: row.user_id, org_id: row.org_id, scope: row.scope })
}
