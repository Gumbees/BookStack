//! Per-org page branding: display name + colors (stored in the settings
//! table under key `branding`, org override → global default → built-in),
//! and logos (bytes in `branding_logos`, org falling back to the global
//! logo).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use super::settings;
use crate::{CoreError, Result};

pub const SETTING_KEY: &str = "branding";
pub const MAX_LOGO_BYTES: usize = 512 * 1024;
pub const ALLOWED_LOGO_MIMES: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];

pub const DEFAULT_PRIMARY: &str = "#206ea7";
pub const DEFAULT_PRIMARY_DARK: &str = "#0f4d79";
pub const DEFAULT_HEADER_TEXT: &str = "#ffffff";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrandingInput {
    pub name: Option<String>,
    pub primary_color: Option<String>,
    pub primary_dark_color: Option<String>,
    pub header_text_color: Option<String>,
}

fn valid_hex(color: &str) -> bool {
    let hex = match color.strip_prefix('#') {
        Some(hex) => hex,
        None => return false,
    };
    (hex.len() == 6 || hex.len() == 3) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn validate(input: &BrandingInput) -> Result<()> {
    for (field, value) in [
        ("primary_color", &input.primary_color),
        ("primary_dark_color", &input.primary_dark_color),
        ("header_text_color", &input.header_text_color),
    ] {
        if let Some(color) = value {
            if !color.is_empty() && !valid_hex(color) {
                return Err(CoreError::validation(format!(
                    "{field} must be a hex color like #1a6ea7"
                )));
            }
        }
    }
    if let Some(name) = &input.name {
        if name.chars().count() > 60 {
            return Err(CoreError::validation("name must be 60 characters or fewer"));
        }
    }
    Ok(())
}

/// Raw stored values at one scope (for the edit forms).
pub async fn get_scope(db: &PgPool, org_id: Option<i64>) -> Result<Value> {
    Ok(settings::get_raw(db, org_id, SETTING_KEY).await?.unwrap_or_else(|| json!({})))
}

/// Store branding at a scope. Empty-string fields clear the override.
pub async fn set_scope(db: &PgPool, org_id: Option<i64>, input: &BrandingInput) -> Result<Value> {
    validate(input)?;
    let mut current = get_scope(db, org_id).await?;
    let object = current.as_object_mut().ok_or_else(|| CoreError::internal("branding not object"))?;
    let mut apply = |key: &str, value: &Option<String>| {
        if let Some(v) = value {
            if v.is_empty() {
                object.remove(key);
            } else {
                object.insert(key.to_string(), json!(v));
            }
        }
    };
    apply("name", &input.name);
    apply("primary_color", &input.primary_color);
    apply("primary_dark_color", &input.primary_dark_color);
    apply("header_text_color", &input.header_text_color);
    settings::set(db, org_id, SETTING_KEY, &current).await?;
    Ok(current)
}

/// Effective branding for a surface: built-in defaults ← global ← org.
pub async fn effective(db: &PgPool, org_id: Option<i64>) -> Result<Value> {
    let global = get_scope(db, None).await?;
    let org = match org_id {
        Some(org) => get_scope(db, Some(org)).await?,
        None => json!({}),
    };
    let pick = |key: &str, fallback: &str| -> Value {
        org.get(key)
            .or_else(|| global.get(key))
            .cloned()
            .unwrap_or_else(|| json!(fallback))
    };
    let logo_scope = logo_scope(db, org_id).await?;
    Ok(json!({
        "name": org.get("name").or_else(|| global.get("name")).cloned().unwrap_or(json!("BookStack")),
        "primary_color": pick("primary_color", DEFAULT_PRIMARY),
        "primary_dark_color": pick("primary_dark_color", DEFAULT_PRIMARY_DARK),
        "header_text_color": pick("header_text_color", DEFAULT_HEADER_TEXT),
        "has_logo": logo_scope.is_some(),
        "logo_scope": logo_scope,
    }))
}

/// Which scope would serve the logo for this org (org / global / none)?
async fn logo_scope(db: &PgPool, org_id: Option<i64>) -> Result<Option<&'static str>> {
    if let Some(org) = org_id {
        let (exists,): (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM branding_logos WHERE org_id = $1)")
                .bind(org)
                .fetch_one(db)
                .await?;
        if exists {
            return Ok(Some("org"));
        }
    }
    let (exists,): (bool,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM branding_logos WHERE org_id IS NULL)")
            .fetch_one(db)
            .await?;
    Ok(exists.then_some("global"))
}

pub async fn set_logo(db: &PgPool, org_id: Option<i64>, mime: &str, data: &[u8]) -> Result<()> {
    if !ALLOWED_LOGO_MIMES.contains(&mime) {
        return Err(CoreError::validation("logo must be image/png, image/jpeg or image/webp"));
    }
    if data.is_empty() || data.len() > MAX_LOGO_BYTES {
        return Err(CoreError::validation("logo must be between 1 byte and 512 KB"));
    }
    sqlx::query(
        "INSERT INTO branding_logos (org_id, mime, data) VALUES ($1, $2, $3)
         ON CONFLICT ((coalesce(org_id, 0)))
         DO UPDATE SET mime = EXCLUDED.mime, data = EXCLUDED.data, updated_at = now()",
    )
    .bind(org_id)
    .bind(mime)
    .bind(data)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_logo(db: &PgPool, org_id: Option<i64>) -> Result<()> {
    match org_id {
        Some(org) => {
            sqlx::query("DELETE FROM branding_logos WHERE org_id = $1").bind(org).execute(db).await?;
        }
        None => {
            sqlx::query("DELETE FROM branding_logos WHERE org_id IS NULL").execute(db).await?;
        }
    }
    Ok(())
}

/// Logo bytes for a surface: the org's logo, else the global one.
pub async fn get_logo(db: &PgPool, org_id: Option<i64>) -> Result<(String, Vec<u8>)> {
    if let Some(org) = org_id {
        let row: Option<(String, Vec<u8>)> =
            sqlx::query_as("SELECT mime, data FROM branding_logos WHERE org_id = $1")
                .bind(org)
                .fetch_optional(db)
                .await?;
        if let Some(found) = row {
            return Ok(found);
        }
    }
    let row: Option<(String, Vec<u8>)> =
        sqlx::query_as("SELECT mime, data FROM branding_logos WHERE org_id IS NULL")
            .fetch_optional(db)
            .await?;
    row.ok_or(CoreError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_validation() {
        assert!(valid_hex("#1a6ea7"));
        assert!(valid_hex("#abc"));
        assert!(!valid_hex("1a6ea7"));
        assert!(!valid_hex("#12345"));
        assert!(!valid_hex("#zzzzzz"));
        assert!(validate(&BrandingInput { primary_color: Some("red".into()), ..Default::default() }).is_err());
        assert!(validate(&BrandingInput { primary_color: Some("#ff0000".into()), ..Default::default() }).is_ok());
    }
}
