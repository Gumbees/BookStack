use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::{books, tags};
use crate::models::{Chapter, ListParams, PageMeta, Paginated, Tag};
use crate::slug::unique_slug;
use crate::{CoreError, Result};

#[derive(Debug, Serialize)]
pub struct ChapterDetails {
    #[serde(flatten)]
    pub chapter: Chapter,
    pub tags: Vec<Tag>,
    pub pages: Vec<PageMeta>,
}

pub async fn list(db: &PgPool, params: &ListParams, book_id: Option<i64>) -> Result<Paginated<Chapter>> {
    let order = params.sort_sql(&["name", "id", "priority", "created_at", "updated_at"]);
    let filter = match book_id {
        Some(_) => "AND book_id = $3",
        None => "",
    };
    let sql = format!(
        "SELECT * FROM chapters WHERE deleted_at IS NULL {filter} ORDER BY {order} LIMIT $1 OFFSET $2"
    );
    let mut query = sqlx::query_as::<_, Chapter>(&sql).bind(params.limit()).bind(params.offset());
    if let Some(id) = book_id {
        query = query.bind(id);
    }
    let data = query.fetch_all(db).await?;

    let count_sql = match book_id {
        Some(_) => "SELECT count(*) FROM chapters WHERE deleted_at IS NULL AND book_id = $1",
        None => "SELECT count(*) FROM chapters WHERE deleted_at IS NULL",
    };
    let mut count_query = sqlx::query_as::<_, (i64,)>(count_sql);
    if let Some(id) = book_id {
        count_query = count_query.bind(id);
    }
    let (total,) = count_query.fetch_one(db).await?;
    Ok(Paginated { data, total })
}

pub async fn fetch(db: &PgPool, id: i64) -> Result<Chapter> {
    sqlx::query_as::<_, Chapter>("SELECT * FROM chapters WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::NotFound)
}

pub async fn get(db: &PgPool, id: i64) -> Result<ChapterDetails> {
    let chapter = fetch(db, id).await?;
    let pages = sqlx::query_as::<_, PageMeta>(
        "SELECT id, book_id, chapter_id, name, slug, priority, draft, revision_count, created_by, updated_by, created_at, updated_at
         FROM pages WHERE chapter_id = $1 AND deleted_at IS NULL ORDER BY priority, id",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    let tags = tags::get_for(db, "chapter", id).await?;
    Ok(ChapterDetails { chapter, tags, pages })
}

async fn next_priority(db: &PgPool, book_id: i64) -> Result<i32> {
    let (max,): (Option<i32>,) = sqlx::query_as(
        "SELECT greatest(
            (SELECT max(priority) FROM chapters WHERE book_id = $1 AND deleted_at IS NULL),
            (SELECT max(priority) FROM pages WHERE book_id = $1 AND chapter_id IS NULL AND deleted_at IS NULL)
        )",
    )
    .bind(book_id)
    .fetch_one(db)
    .await?;
    Ok(max.unwrap_or(0) + 1)
}

#[derive(Debug, Deserialize)]
pub struct CreateChapter {
    pub book_id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

pub async fn create(db: &PgPool, user_id: i64, input: &CreateChapter) -> Result<ChapterDetails> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(CoreError::validation("name is required"));
    }
    books::fetch(db, input.book_id).await?;
    let book_id = input.book_id;
    let slug = unique_slug(name, |candidate| async move {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM chapters WHERE book_id = $1 AND slug = $2 AND deleted_at IS NULL)",
        )
        .bind(book_id)
        .bind(candidate)
        .fetch_one(db)
        .await?;
        Ok(exists)
    })
    .await?;
    let priority = next_priority(db, book_id).await?;

    let chapter = sqlx::query_as::<_, Chapter>(
        "INSERT INTO chapters (book_id, name, slug, description, priority, created_by, updated_by)
         VALUES ($1, $2, $3, $4, $5, $6, $6) RETURNING *",
    )
    .bind(book_id)
    .bind(name)
    .bind(&slug)
    .bind(input.description.trim())
    .bind(priority)
    .bind(user_id)
    .fetch_one(db)
    .await?;
    if !input.tags.is_empty() {
        tags::set_for(db, "chapter", chapter.id, &input.tags).await?;
    }
    get(db, chapter.id).await
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateChapter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub tags: Option<Vec<Tag>>,
}

pub async fn update(db: &PgPool, user_id: i64, id: i64, input: &UpdateChapter) -> Result<ChapterDetails> {
    let current = fetch(db, id).await?;
    let name = input.name.clone().unwrap_or(current.name);
    let description = input.description.clone().unwrap_or(current.description);
    let priority = input.priority.unwrap_or(current.priority);
    sqlx::query(
        "UPDATE chapters SET name = $2, description = $3, priority = $4, updated_by = $5, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(name.trim())
    .bind(description.trim())
    .bind(priority)
    .bind(user_id)
    .execute(db)
    .await?;
    if let Some(t) = &input.tags {
        tags::set_for(db, "chapter", id, t).await?;
    }
    get(db, id).await
}

/// Soft-delete a chapter; contained pages move to the book root.
pub async fn delete(db: &PgPool, id: i64) -> Result<()> {
    let mut tx = db.begin().await?;
    let res = sqlx::query("UPDATE chapters SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    sqlx::query("UPDATE pages SET chapter_id = NULL WHERE chapter_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
