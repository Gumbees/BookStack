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

pub async fn info(db: &PgPool) -> Result<SystemInfo> {
    let (shelves, books, chapters, pages, users): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM shelves WHERE deleted_at IS NULL),
            (SELECT count(*) FROM books WHERE deleted_at IS NULL),
            (SELECT count(*) FROM chapters WHERE deleted_at IS NULL),
            (SELECT count(*) FROM pages WHERE deleted_at IS NULL),
            (SELECT count(*) FROM users)",
    )
    .fetch_one(db)
    .await?;
    Ok(SystemInfo {
        name: "BookStack-rs".to_string(),
        version: VERSION.to_string(),
        database: "postgresql".to_string(),
        counts: Counts { shelves, books, chapters, pages, users },
    })
}
