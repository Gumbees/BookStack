use serde::Serialize;
use sqlx::PgPool;

use crate::Result;

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub name: String,
    pub version: String,
    pub database: String,
    pub counts: Counts,
}

#[derive(Debug, Serialize)]
pub struct Counts {
    pub shelves: i64,
    pub books: i64,
    pub chapters: i64,
    pub pages: i64,
    pub users: i64,
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn info(db: &PgPool, org_id: i64) -> Result<SystemInfo> {
    let (shelves, books, chapters, pages, users): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM shelves WHERE org_id = $1 AND deleted_at IS NULL),
            (SELECT count(*) FROM books WHERE org_id = $1 AND deleted_at IS NULL),
            (SELECT count(*) FROM chapters WHERE org_id = $1 AND deleted_at IS NULL),
            (SELECT count(*) FROM pages WHERE org_id = $1 AND deleted_at IS NULL),
            (SELECT count(*) FROM org_members WHERE org_id = $1)",
    )
    .bind(org_id)
    .fetch_one(db)
    .await?;
    Ok(SystemInfo {
        name: "BookStack-rs".to_string(),
        version: VERSION.to_string(),
        database: "postgresql".to_string(),
        counts: Counts { shelves, books, chapters, pages, users },
    })
}
