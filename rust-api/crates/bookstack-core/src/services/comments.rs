use sqlx::PgPool;

use super::pages;
use crate::markdown;
use crate::models::Comment;
use crate::{CoreError, Result};

pub async fn list(db: &PgPool, org_id: i64, page_id: Option<i64>) -> Result<Vec<Comment>> {
    let comments = match page_id {
        Some(pid) => {
            sqlx::query_as::<_, Comment>(
                "SELECT c.* FROM comments c JOIN pages p ON p.id = c.page_id
                 WHERE c.page_id = $1 AND p.org_id = $2 ORDER BY c.id",
            )
            .bind(pid)
            .bind(org_id)
            .fetch_all(db)
            .await?
        }
        None => {
            sqlx::query_as::<_, Comment>(
                "SELECT c.* FROM comments c JOIN pages p ON p.id = c.page_id
                 WHERE p.org_id = $1 ORDER BY c.id DESC LIMIT 500",
            )
            .bind(org_id)
            .fetch_all(db)
            .await?
        }
    };
    Ok(comments)
}

pub async fn get(db: &PgPool, org_id: i64, id: i64) -> Result<Comment> {
    sqlx::query_as::<_, Comment>(
        "SELECT c.* FROM comments c JOIN pages p ON p.id = c.page_id WHERE c.id = $1 AND p.org_id = $2",
    )
    .bind(id)
    .bind(org_id)
    .fetch_optional(db)
    .await?
    .ok_or(CoreError::NotFound)
}

pub async fn create(
    db: &PgPool,
    org_id: i64,
    user_id: i64,
    page_id: i64,
    markdown_body: &str,
    parent_id: Option<i64>,
) -> Result<Comment> {
    pages::fetch(db, org_id, page_id).await?;
    if let Some(parent) = parent_id {
        let parent_comment = get(db, org_id, parent).await?;
        if parent_comment.page_id != page_id {
            return Err(CoreError::validation("parent comment belongs to a different page"));
        }
    }
    let html = markdown::render(markdown_body);
    Ok(sqlx::query_as::<_, Comment>(
        "INSERT INTO comments (page_id, parent_id, markdown, html, created_by, updated_by)
         VALUES ($1, $2, $3, $4, $5, $5) RETURNING *",
    )
    .bind(page_id)
    .bind(parent_id)
    .bind(markdown_body)
    .bind(&html)
    .bind(user_id)
    .fetch_one(db)
    .await?)
}

pub async fn update(db: &PgPool, org_id: i64, user_id: i64, id: i64, markdown_body: &str) -> Result<Comment> {
    get(db, org_id, id).await?;
    let html = markdown::render(markdown_body);
    Ok(sqlx::query_as::<_, Comment>(
        "UPDATE comments SET markdown = $2, html = $3, updated_by = $4, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(markdown_body)
    .bind(&html)
    .bind(user_id)
    .fetch_one(db)
    .await?)
}

pub async fn delete(db: &PgPool, org_id: i64, id: i64) -> Result<()> {
    get(db, org_id, id).await?;
    let res = sqlx::query("DELETE FROM comments WHERE id = $1").bind(id).execute(db).await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}
