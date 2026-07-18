//! Recycle bin over soft deletes. Every delete records a `deletions` row;
//! restore/destroy consume it. Children soft-deleted in the same transaction
//! share the parent's `deleted_at` (Postgres `now()` is
//! transaction-stable), which restore uses to bring them back together.

use sqlx::{PgPool, Postgres, Transaction};

use crate::models::{Deletion, ListParams, Paginated};
use crate::{CoreError, Result};

pub(crate) async fn record(
    tx: &mut Transaction<'_, Postgres>,
    entity_type: &str,
    entity_id: i64,
    entity_name: &str,
    user_id: Option<i64>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO deletions (entity_type, entity_id, entity_name, deleted_by, deleted_at)
         VALUES ($1, $2, $3, $4, now())",
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(entity_name)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn list(db: &PgPool, params: &ListParams) -> Result<Paginated<Deletion>> {
    let data = sqlx::query_as::<_, Deletion>(
        "SELECT * FROM deletions ORDER BY deleted_at DESC, id DESC LIMIT $1 OFFSET $2",
    )
    .bind(params.limit())
    .bind(params.offset())
    .fetch_all(db)
    .await?;
    let (total,): (i64,) = sqlx::query_as("SELECT count(*) FROM deletions").fetch_one(db).await?;
    Ok(Paginated { data, total })
}

async fn fetch(db: &PgPool, deletion_id: i64) -> Result<Deletion> {
    sqlx::query_as::<_, Deletion>("SELECT * FROM deletions WHERE id = $1")
        .bind(deletion_id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::NotFound)
}

/// Restore a soft-deleted entity (and, for books, the children deleted with it).
pub async fn restore(db: &PgPool, deletion_id: i64) -> Result<Deletion> {
    let deletion = fetch(db, deletion_id).await?;
    let mut tx = db.begin().await?;
    match deletion.entity_type.as_str() {
        "shelf" => {
            sqlx::query("UPDATE shelves SET deleted_at = NULL WHERE id = $1")
                .bind(deletion.entity_id)
                .execute(&mut *tx)
                .await?;
        }
        "book" => {
            // Children first: rows sharing the book's deleted_at timestamp.
            sqlx::query(
                "UPDATE chapters SET deleted_at = NULL
                 WHERE book_id = $1 AND deleted_at = (SELECT deleted_at FROM books WHERE id = $1)",
            )
            .bind(deletion.entity_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE pages SET deleted_at = NULL
                 WHERE book_id = $1 AND deleted_at = (SELECT deleted_at FROM books WHERE id = $1)",
            )
            .bind(deletion.entity_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE books SET deleted_at = NULL WHERE id = $1")
                .bind(deletion.entity_id)
                .execute(&mut *tx)
                .await?;
        }
        "chapter" => {
            let parent_alive: Option<(bool,)> = sqlx::query_as(
                "SELECT b.deleted_at IS NULL FROM chapters c JOIN books b ON b.id = c.book_id WHERE c.id = $1",
            )
            .bind(deletion.entity_id)
            .fetch_optional(&mut *tx)
            .await?;
            if !matches!(parent_alive, Some((true,))) {
                return Err(CoreError::validation(
                    "the chapter's parent book is deleted; restore the book first",
                ));
            }
            sqlx::query("UPDATE chapters SET deleted_at = NULL WHERE id = $1")
                .bind(deletion.entity_id)
                .execute(&mut *tx)
                .await?;
        }
        _ => {
            let parent_alive: Option<(bool,)> = sqlx::query_as(
                "SELECT b.deleted_at IS NULL FROM pages p JOIN books b ON b.id = p.book_id WHERE p.id = $1",
            )
            .bind(deletion.entity_id)
            .fetch_optional(&mut *tx)
            .await?;
            if !matches!(parent_alive, Some((true,))) {
                return Err(CoreError::validation(
                    "the page's parent book is deleted; restore the book first",
                ));
            }
            // Detach from its chapter if that chapter is still deleted.
            sqlx::query(
                "UPDATE pages SET deleted_at = NULL,
                        chapter_id = CASE
                            WHEN chapter_id IS NOT NULL AND EXISTS (
                                SELECT 1 FROM chapters c WHERE c.id = chapter_id AND c.deleted_at IS NOT NULL
                            ) THEN NULL
                            ELSE chapter_id
                        END
                 WHERE id = $1",
            )
            .bind(deletion.entity_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    sqlx::query("DELETE FROM deletions WHERE id = $1")
        .bind(deletion_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(deletion)
}

/// Permanently delete a recycle-bin entry's entity. Cannot be undone.
pub async fn destroy(db: &PgPool, deletion_id: i64) -> Result<Deletion> {
    let deletion = fetch(db, deletion_id).await?;
    let mut tx = db.begin().await?;
    let table = match deletion.entity_type.as_str() {
        "shelf" => "shelves",
        "book" => "books",
        "chapter" => "chapters",
        _ => "pages",
    };
    // Books cascade to chapters/pages via FK; clean up their deletion rows too.
    if deletion.entity_type == "book" {
        sqlx::query(
            "DELETE FROM deletions d
             WHERE (d.entity_type = 'chapter' AND d.entity_id IN (SELECT id FROM chapters WHERE book_id = $1))
                OR (d.entity_type = 'page' AND d.entity_id IN (SELECT id FROM pages WHERE book_id = $1))",
        )
        .bind(deletion.entity_id)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(&format!("DELETE FROM {table} WHERE id = $1"))
        .bind(deletion.entity_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM deletions WHERE id = $1")
        .bind(deletion_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(deletion)
}
