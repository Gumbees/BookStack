//! Encrypted backups. Scopes: `org` (one org's content), `global` (whole
//! instance), `sql` (pg_dump logical snapshot). Policy: object storage for
//! any scope; the filesystem target is restricted to global/sql backups (the
//! API layer enforces it before calling in).

use serde_json::{json, Value};
use sqlx::PgPool;

use crate::storage::{Storage, Target};
use crate::{crypto, OpsError, Result};

async fn table_dump(db: &PgPool, query: &str, bind_org: Option<i64>) -> Result<Value> {
    let rows: Vec<(Value,)> = match bind_org {
        Some(org) => sqlx::query_as(query).bind(org).fetch_all(db).await?,
        None => sqlx::query_as(query).fetch_all(db).await?,
    };
    Ok(Value::Array(rows.into_iter().map(|(v,)| v).collect()))
}

/// One org's content as JSON (schema-versioned envelope).
pub async fn dump_org(db: &PgPool, org_id: i64) -> Result<Value> {
    Ok(json!({
        "format": "bookstack-rs.org-backup",
        "version": 1,
        "org": table_dump(db, "SELECT to_jsonb(o) FROM orgs o WHERE id = $1", Some(org_id)).await?,
        "members": table_dump(db, "SELECT to_jsonb(m) FROM org_members m WHERE org_id = $1", Some(org_id)).await?,
        "shelves": table_dump(db, "SELECT to_jsonb(s) - 'search_vector' FROM shelves s WHERE org_id = $1", Some(org_id)).await?,
        "shelf_books": table_dump(db, "SELECT to_jsonb(sb) FROM shelf_books sb JOIN shelves s ON s.id = sb.shelf_id WHERE s.org_id = $1", Some(org_id)).await?,
        "books": table_dump(db, "SELECT to_jsonb(b) - 'search_vector' FROM books b WHERE org_id = $1", Some(org_id)).await?,
        "chapters": table_dump(db, "SELECT to_jsonb(c) - 'search_vector' FROM chapters c WHERE org_id = $1", Some(org_id)).await?,
        "pages": table_dump(db, "SELECT to_jsonb(p) - 'search_vector' - 'ydoc_state' FROM pages p WHERE org_id = $1", Some(org_id)).await?,
        "page_revisions": table_dump(db, "SELECT to_jsonb(r) FROM page_revisions r JOIN pages p ON p.id = r.page_id WHERE p.org_id = $1", Some(org_id)).await?,
        "comments": table_dump(db, "SELECT to_jsonb(c) FROM comments c JOIN pages p ON p.id = c.page_id WHERE p.org_id = $1", Some(org_id)).await?,
        "tags": table_dump(db, "SELECT to_jsonb(t) FROM tags t WHERE (t.entity_type = 'shelf' AND t.entity_id IN (SELECT id FROM shelves WHERE org_id = $1)) OR (t.entity_type = 'book' AND t.entity_id IN (SELECT id FROM books WHERE org_id = $1)) OR (t.entity_type = 'chapter' AND t.entity_id IN (SELECT id FROM chapters WHERE org_id = $1)) OR (t.entity_type = 'page' AND t.entity_id IN (SELECT id FROM pages WHERE org_id = $1))", Some(org_id)).await?,
        "deletions": table_dump(db, "SELECT to_jsonb(d) FROM deletions d WHERE org_id = $1", Some(org_id)).await?,
    }))
}

/// Whole-instance content dump (all orgs, users, settings, providers).
pub async fn dump_global(db: &PgPool) -> Result<Value> {
    Ok(json!({
        "format": "bookstack-rs.global-backup",
        "version": 1,
        "users": table_dump(db, "SELECT to_jsonb(u) FROM users u", None).await?,
        "orgs": table_dump(db, "SELECT to_jsonb(o) FROM orgs o", None).await?,
        "org_members": table_dump(db, "SELECT to_jsonb(m) FROM org_members m", None).await?,
        "settings": table_dump(db, "SELECT to_jsonb(s) FROM settings s", None).await?,
        "auth_providers": table_dump(db, "SELECT to_jsonb(a) FROM auth_providers a", None).await?,
        "api_tokens": table_dump(db, "SELECT to_jsonb(t) FROM api_tokens t", None).await?,
        "shelves": table_dump(db, "SELECT to_jsonb(s) - 'search_vector' FROM shelves s", None).await?,
        "shelf_books": table_dump(db, "SELECT to_jsonb(sb) FROM shelf_books sb", None).await?,
        "books": table_dump(db, "SELECT to_jsonb(b) - 'search_vector' FROM books b", None).await?,
        "chapters": table_dump(db, "SELECT to_jsonb(c) - 'search_vector' FROM chapters c", None).await?,
        "pages": table_dump(db, "SELECT to_jsonb(p) - 'search_vector' - 'ydoc_state' FROM pages p", None).await?,
        "page_revisions": table_dump(db, "SELECT to_jsonb(r) FROM page_revisions r", None).await?,
        "comments": table_dump(db, "SELECT to_jsonb(c) FROM comments c", None).await?,
        "tags": table_dump(db, "SELECT to_jsonb(t) FROM tags t", None).await?,
        "deletions": table_dump(db, "SELECT to_jsonb(d) FROM deletions d", None).await?,
    }))
}

/// Logical SQL snapshot via pg_dump (plain format, gzipped + encrypted).
pub async fn dump_sql(database_url: &str) -> Result<Vec<u8>> {
    let output = tokio::process::Command::new("pg_dump")
        .arg("--no-owner")
        .arg("--no-privileges")
        .arg(database_url)
        .output()
        .await
        .map_err(|e| OpsError::Config(format!("pg_dump not runnable: {e}")))?;
    if !output.status.success() {
        return Err(OpsError::Storage(format!(
            "pg_dump failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
}

pub fn passphrase() -> Result<String> {
    std::env::var("BACKUP_ENCRYPTION_KEY").ok().filter(|k| k.len() >= 12).ok_or_else(|| {
        OpsError::Config(
            "BACKUP_ENCRYPTION_KEY (min 12 chars) must be set — backups are always encrypted".into(),
        )
    })
}

pub struct BackupOutcome {
    pub location: String,
    pub size_bytes: i64,
    pub object_name: String,
}

/// Produce, compress, encrypt and store one backup.
pub async fn run(
    db: &PgPool,
    database_url: &str,
    scope: &str,
    org_id: Option<i64>,
    target: Target,
) -> Result<BackupOutcome> {
    let key = passphrase()?;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let (name, plaintext) = match scope {
        "org" => {
            let org = org_id.ok_or_else(|| OpsError::Config("org_id required for org backups".into()))?;
            let dump = dump_org(db, org).await?;
            (
                format!("org-{org}/bookstack-org{org}-{timestamp}.json.gz.enc"),
                serde_json::to_vec(&dump).map_err(|e| OpsError::Storage(e.to_string()))?,
            )
        }
        "global" => {
            let dump = dump_global(db).await?;
            (
                format!("global/bookstack-global-{timestamp}.json.gz.enc"),
                serde_json::to_vec(&dump).map_err(|e| OpsError::Storage(e.to_string()))?,
            )
        }
        "sql" => (
            format!("sql/bookstack-{timestamp}.sql.gz.enc"),
            dump_sql(database_url).await?,
        ),
        other => return Err(OpsError::Config(format!("unknown backup scope '{other}'"))),
    };

    let sealed = crypto::encrypt(&key, &crypto::gzip(&plaintext)?)?;
    let size = sealed.len() as i64;
    let storage = Storage::for_target(target)?;
    let location = storage.put(&name, sealed).await?;
    Ok(BackupOutcome { location, size_bytes: size, object_name: name })
}

/// Download, decrypt and sanity-check a stored backup; returns summary info.
pub async fn verify(target: Target, object_name: &str, scope: &str) -> Result<Value> {
    let key = passphrase()?;
    let storage = Storage::for_target(target)?;
    let sealed = storage.get(object_name).await?;
    let plaintext = crypto::gunzip(&crypto::decrypt(&key, &sealed)?)?;
    match scope {
        "sql" => Ok(json!({
            "ok": true,
            "decrypted_bytes": plaintext.len(),
            "looks_like_sql": plaintext.starts_with(b"--") || plaintext.windows(6).take(2000).any(|w| w == b"CREATE"),
        })),
        _ => {
            let parsed: Value =
                serde_json::from_slice(&plaintext).map_err(|e| OpsError::Crypto(format!("backup JSON invalid: {e}")))?;
            Ok(json!({
                "ok": true,
                "decrypted_bytes": plaintext.len(),
                "format": parsed.get("format"),
                "counts": {
                    "books": parsed.get("books").and_then(|v| v.as_array()).map(|a| a.len()),
                    "pages": parsed.get("pages").and_then(|v| v.as_array()).map(|a| a.len()),
                    "users": parsed.get("users").and_then(|v| v.as_array()).map(|a| a.len()),
                },
            }))
        }
    }
}
