use argon2::password_hash::{rand_core::OsRng, PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::models::{ApiToken, AuthUser, User};
use crate::{CoreError, Result};

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(CoreError::internal)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: i64,
    name: String,
    role: String,
    exp: i64,
}

pub fn issue_jwt(secret: &str, user: &User) -> Result<String> {
    let claims = Claims {
        sub: user.id,
        name: user.name.clone(),
        role: user.role.clone(),
        exp: (Utc::now() + Duration::days(7)).timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(CoreError::internal)
}

/// Validate a JWT and load the current user record.
pub async fn verify_jwt(db: &PgPool, secret: &str, token: &str) -> Result<AuthUser> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| CoreError::Unauthorized)?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(data.claims.sub)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::Unauthorized)?;
    Ok(AuthUser::from(&user))
}

pub async fn login(db: &PgPool, secret: &str, email: &str, password: &str) -> Result<(String, AuthUser)> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE lower(email) = lower($1)")
        .bind(email)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::Unauthorized)?;
    if !verify_password(password, &user.password_hash) {
        return Err(CoreError::Unauthorized);
    }
    let token = issue_jwt(secret, &user)?;
    Ok((token, AuthUser::from(&user)))
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

/// Freshly created API token, including the secret (shown exactly once).
#[derive(Debug, Serialize)]
pub struct CreatedToken {
    pub id: i64,
    pub name: String,
    pub token_id: String,
    pub secret: String,
}

/// Create a BookStack-style API token (`Authorization: Token <id>:<secret>`).
pub async fn create_api_token(db: &PgPool, user_id: i64, name: &str) -> Result<CreatedToken> {
    let token_id = random_token(32);
    let secret = random_token(32);
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO api_tokens (user_id, name, token_id, secret_hash) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .bind(&token_id)
    .bind(sha256_hex(&secret))
    .fetch_one(db)
    .await?;
    Ok(CreatedToken {
        id: row.0,
        name: name.to_string(),
        token_id,
        secret,
    })
}

pub async fn list_api_tokens(db: &PgPool, user_id: i64) -> Result<Vec<ApiToken>> {
    Ok(
        sqlx::query_as::<_, ApiToken>("SELECT * FROM api_tokens WHERE user_id = $1 ORDER BY id")
            .bind(user_id)
            .fetch_all(db)
            .await?,
    )
}

pub async fn delete_api_token(db: &PgPool, user_id: i64, token_pk: i64) -> Result<()> {
    let res = sqlx::query("DELETE FROM api_tokens WHERE id = $1 AND user_id = $2")
        .bind(token_pk)
        .bind(user_id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Validate an `Authorization: Token id:secret` header value.
pub async fn verify_api_token(db: &PgPool, header_value: &str) -> Result<AuthUser> {
    let (token_id, secret) = header_value
        .split_once(':')
        .ok_or(CoreError::Unauthorized)?;
    let token = sqlx::query_as::<_, ApiToken>("SELECT * FROM api_tokens WHERE token_id = $1")
        .bind(token_id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::Unauthorized)?;
    if token.secret_hash != sha256_hex(secret) {
        return Err(CoreError::Unauthorized);
    }
    if let Some(expires) = token.expires_at {
        if expires < Utc::now() {
            return Err(CoreError::Unauthorized);
        }
    }
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(token.user_id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::Unauthorized)?;
    sqlx::query("UPDATE api_tokens SET last_used_at = now() WHERE id = $1")
        .bind(token.id)
        .execute(db)
        .await
        .ok();
    Ok(AuthUser::from(&user))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let hash = hash_password("s3cret!").unwrap();
        assert!(verify_password("s3cret!", &hash));
        assert!(!verify_password("wrong", &hash));
    }
}
