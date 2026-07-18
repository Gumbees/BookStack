use serde::Deserialize;
use sqlx::PgPool;

use super::map_unique;
use crate::models::{Org, OrgMemberInfo, OrgMembership, Role};
use crate::slug::slugify;
use crate::{CoreError, Result};

pub async fn get(db: &PgPool, id: i64) -> Result<Org> {
    sqlx::query_as::<_, Org>("SELECT * FROM orgs WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::NotFound)
}

pub async fn list_all(db: &PgPool) -> Result<Vec<Org>> {
    Ok(sqlx::query_as::<_, Org>("SELECT * FROM orgs ORDER BY name").fetch_all(db).await?)
}

/// Orgs the user belongs to, with their per-org role.
pub async fn memberships(db: &PgPool, user_id: i64) -> Result<Vec<OrgMembership>> {
    Ok(sqlx::query_as::<_, OrgMembership>(
        "SELECT o.id AS org_id, o.name, o.slug, m.role
         FROM org_members m JOIN orgs o ON o.id = m.org_id
         WHERE m.user_id = $1 ORDER BY o.name",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?)
}

/// The user's effective role inside an org. System admins (instance-level
/// `users.role = 'admin'`) act as admin in every org.
pub async fn role_in(db: &PgPool, user_id: i64, org_id: i64, system_admin: bool) -> Result<Role> {
    if system_admin {
        get(db, org_id).await?;
        return Ok(Role::Admin);
    }
    let row: Option<(String,)> =
        sqlx::query_as("SELECT role FROM org_members WHERE org_id = $1 AND user_id = $2")
            .bind(org_id)
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    match row {
        Some((role,)) => Ok(Role::parse(&role).unwrap_or(Role::Viewer)),
        None => Err(CoreError::Forbidden),
    }
}

/// Org ids the user can read (all orgs for system admins).
pub async fn accessible_org_ids(db: &PgPool, user_id: i64, system_admin: bool) -> Result<Vec<i64>> {
    let rows: Vec<(i64,)> = if system_admin {
        sqlx::query_as("SELECT id FROM orgs ORDER BY id").fetch_all(db).await?
    } else {
        sqlx::query_as("SELECT org_id FROM org_members WHERE user_id = $1 ORDER BY org_id")
            .bind(user_id)
            .fetch_all(db)
            .await?
    };
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

#[derive(Debug, Deserialize)]
pub struct CreateOrg {
    pub name: String,
}

/// Create an org; the creator becomes its admin.
pub async fn create(db: &PgPool, creator_id: i64, input: &CreateOrg) -> Result<Org> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(CoreError::validation("name is required"));
    }
    let base = slugify(name);
    let mut slug = base.clone();
    for n in 2..50 {
        let (exists,): (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM orgs WHERE slug = $1)")
            .bind(&slug)
            .fetch_one(db)
            .await?;
        if !exists {
            break;
        }
        slug = format!("{base}-{n}");
    }
    let org = sqlx::query_as::<_, Org>("INSERT INTO orgs (name, slug) VALUES ($1, $2) RETURNING *")
        .bind(name)
        .bind(&slug)
        .fetch_one(db)
        .await
        .map_err(|e| map_unique(e, "an org with that slug already exists"))?;
    sqlx::query("INSERT INTO org_members (org_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(org.id)
        .bind(creator_id)
        .execute(db)
        .await?;
    Ok(org)
}

pub async fn members(db: &PgPool, org_id: i64) -> Result<Vec<OrgMemberInfo>> {
    Ok(sqlx::query_as::<_, OrgMemberInfo>(
        "SELECT u.id AS user_id, u.name, u.email, m.role
         FROM org_members m JOIN users u ON u.id = m.user_id
         WHERE m.org_id = $1 ORDER BY u.name",
    )
    .bind(org_id)
    .fetch_all(db)
    .await?)
}

/// Add (or update) a member by email with a role.
pub async fn upsert_member(db: &PgPool, org_id: i64, email: &str, role: Role) -> Result<OrgMemberInfo> {
    let user: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE lower(email) = lower($1)")
        .bind(email)
        .fetch_optional(db)
        .await?;
    let (user_id,) = user.ok_or_else(|| CoreError::validation("no user with that email exists"))?;
    sqlx::query(
        "INSERT INTO org_members (org_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (org_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(org_id)
    .bind(user_id)
    .bind(role.as_str())
    .execute(db)
    .await?;
    let member = members(db, org_id)
        .await?
        .into_iter()
        .find(|m| m.user_id == user_id)
        .ok_or(CoreError::NotFound)?;
    Ok(member)
}

/// Ensure a user is at least a member (used when an org-scoped SSO provider
/// logs someone in). Existing roles are never downgraded.
pub async fn ensure_member(db: &PgPool, org_id: i64, user_id: i64, role: Role) -> Result<()> {
    sqlx::query(
        "INSERT INTO org_members (org_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (org_id, user_id) DO NOTHING",
    )
    .bind(org_id)
    .bind(user_id)
    .bind(role.as_str())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove_member(db: &PgPool, org_id: i64, user_id: i64) -> Result<()> {
    let (admins,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM org_members WHERE org_id = $1 AND role = 'admin'")
            .bind(org_id)
            .fetch_one(db)
            .await?;
    let target: Option<(String,)> =
        sqlx::query_as("SELECT role FROM org_members WHERE org_id = $1 AND user_id = $2")
            .bind(org_id)
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    let Some((target_role,)) = target else {
        return Err(CoreError::NotFound);
    };
    if target_role == "admin" && admins <= 1 {
        return Err(CoreError::validation("cannot remove the last admin of an org"));
    }
    sqlx::query("DELETE FROM org_members WHERE org_id = $1 AND user_id = $2")
        .bind(org_id)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}
