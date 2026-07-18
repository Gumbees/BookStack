use serde::Deserialize;
use sqlx::PgPool;

use super::map_unique;
use crate::auth::hash_password;
use crate::models::{ListParams, Paginated, Role, User};
use crate::{CoreError, Result};

pub async fn list(db: &PgPool, params: &ListParams) -> Result<Paginated<User>> {
    let order = params.sort_sql(&["id", "name", "email", "created_at"]);
    let data = sqlx::query_as::<_, User>(&format!(
        "SELECT * FROM users ORDER BY {order} LIMIT $1 OFFSET $2"
    ))
    .bind(params.limit())
    .bind(params.offset())
    .fetch_all(db)
    .await?;
    let (total,): (i64,) = sqlx::query_as("SELECT count(*) FROM users").fetch_one(db).await?;
    Ok(Paginated { data, total })
}

pub async fn get(db: &PgPool, id: i64) -> Result<User> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::NotFound)
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub name: String,
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub role: Option<String>,
}

pub async fn create(db: &PgPool, input: &CreateUser) -> Result<User> {
    if input.password.len() < 8 {
        return Err(CoreError::validation("password must be at least 8 characters"));
    }
    let role = match &input.role {
        Some(r) => Role::parse(r)
            .ok_or_else(|| CoreError::validation("role must be admin, editor or viewer"))?,
        None => Role::Viewer,
    };
    let hash = hash_password(&input.password)?;
    sqlx::query_as::<_, User>(
        "INSERT INTO users (name, email, password_hash, role) VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(input.name.trim())
    .bind(input.email.trim())
    .bind(&hash)
    .bind(role.as_str())
    .fetch_one(db)
    .await
    .map_err(|e| map_unique(e, "a user with that email already exists"))
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateUser {
    pub name: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
}

pub async fn update(db: &PgPool, id: i64, input: &UpdateUser) -> Result<User> {
    let current = get(db, id).await?;
    let name = input.name.clone().unwrap_or(current.name);
    let email = input.email.clone().unwrap_or(current.email);
    let role = match &input.role {
        Some(r) => Role::parse(r)
            .ok_or_else(|| CoreError::validation("role must be admin, editor or viewer"))?
            .as_str()
            .to_string(),
        None => current.role,
    };
    let password_hash = match &input.password {
        Some(p) if !p.is_empty() => {
            if p.len() < 8 {
                return Err(CoreError::validation("password must be at least 8 characters"));
            }
            hash_password(p)?
        }
        _ => current.password_hash,
    };
    sqlx::query_as::<_, User>(
        "UPDATE users SET name = $2, email = $3, role = $4, password_hash = $5, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(name.trim())
    .bind(email.trim())
    .bind(role)
    .bind(password_hash)
    .fetch_one(db)
    .await
    .map_err(|e| map_unique(e, "a user with that email already exists"))
}

pub async fn delete(db: &PgPool, id: i64) -> Result<()> {
    let (admins,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE role = 'admin'")
        .fetch_one(db)
        .await?;
    let target = get(db, id).await?;
    if target.role == "admin" && admins <= 1 {
        return Err(CoreError::validation("cannot delete the last admin user"));
    }
    sqlx::query("DELETE FROM users WHERE id = $1").bind(id).execute(db).await?;
    Ok(())
}

/// Seed the initial admin account when the users table is empty.
/// Returns the credentials when a user was created.
pub async fn ensure_admin(db: &PgPool, email: &str, password: &str) -> Result<Option<(String, String)>> {
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM users").fetch_one(db).await?;
    if count > 0 {
        return Ok(None);
    }
    let user = create(
        db,
        &CreateUser {
            name: "Admin".to_string(),
            email: email.to_string(),
            password: password.to_string(),
            role: Some("admin".to_string()),
        },
    )
    .await?;
    sqlx::query(
        "INSERT INTO org_members (org_id, user_id, role) VALUES (1, $1, 'admin') ON CONFLICT DO NOTHING",
    )
    .bind(user.id)
    .execute(db)
    .await?;
    Ok(Some((email.to_string(), password.to_string())))
}
