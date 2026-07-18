use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::tags;
use crate::models::{Book, Chapter, ContentItem, ListParams, PageMeta, Paginated, Tag};
use crate::slug::unique_slug;
use crate::{CoreError, Result};

#[derive(Debug, Serialize)]
pub struct BookDetails {
    #[serde(flatten)]
    pub book: Book,
    pub tags: Vec<Tag>,
    pub contents: Vec<ContentItem>,
}

pub async fn list(db: &PgPool, params: &ListParams) -> Result<Paginated<Book>> {
    let order = params.sort_sql(&["name", "id", "created_at", "updated_at"]);
    let data = sqlx::query_as::<_, Book>(&format!(
        "SELECT * FROM books WHERE deleted_at IS NULL ORDER BY {order} LIMIT $1 OFFSET $2"
    ))
    .bind(params.limit())
    .bind(params.offset())
    .fetch_all(db)
    .await?;
    let (total,): (i64,) = sqlx::query_as("SELECT count(*) FROM books WHERE deleted_at IS NULL")
        .fetch_one(db)
        .await?;
    Ok(Paginated { data, total })
}

pub async fn fetch(db: &PgPool, id: i64) -> Result<Book> {
    sqlx::query_as::<_, Book>("SELECT * FROM books WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::NotFound)
}

pub async fn get(db: &PgPool, id: i64) -> Result<BookDetails> {
    let book = fetch(db, id).await?;
    let tags = tags::get_for(db, "book", id).await?;
    let contents = contents(db, id).await?;
    Ok(BookDetails { book, tags, contents })
}

pub async fn get_by_slug(db: &PgPool, slug: &str) -> Result<BookDetails> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM books WHERE slug = $1 AND deleted_at IS NULL")
            .bind(slug)
            .fetch_optional(db)
            .await?;
    match row {
        Some((id,)) => get(db, id).await,
        None => Err(CoreError::NotFound),
    }
}

/// Build the ordered contents tree of a book (chapters with nested pages,
/// plus top-level pages), matching BookStack's book view.
pub async fn contents(db: &PgPool, book_id: i64) -> Result<Vec<ContentItem>> {
    let chapters = sqlx::query_as::<_, Chapter>(
        "SELECT * FROM chapters WHERE book_id = $1 AND deleted_at IS NULL ORDER BY priority, id",
    )
    .bind(book_id)
    .fetch_all(db)
    .await?;
    let pages = sqlx::query_as::<_, PageMeta>(
        "SELECT id, book_id, chapter_id, name, slug, priority, draft, revision_count, created_by, updated_by, created_at, updated_at
         FROM pages WHERE book_id = $1 AND deleted_at IS NULL ORDER BY priority, id",
    )
    .bind(book_id)
    .fetch_all(db)
    .await?;

    let mut items: Vec<(i32, i64, ContentItem)> = Vec::new();
    for chapter in chapters {
        let chapter_pages: Vec<PageMeta> = pages
            .iter()
            .filter(|p| p.chapter_id == Some(chapter.id))
            .cloned()
            .collect();
        items.push((
            chapter.priority,
            chapter.id,
            ContentItem::Chapter { chapter, pages: chapter_pages },
        ));
    }
    for page in pages.into_iter().filter(|p| p.chapter_id.is_none()) {
        items.push((page.priority, page.id, ContentItem::Page { page }));
    }
    items.sort_by_key(|(priority, id, _)| (*priority, *id));
    Ok(items.into_iter().map(|(_, _, item)| item).collect())
}

#[derive(Debug, Deserialize)]
pub struct CreateBook {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

pub async fn create(db: &PgPool, user_id: i64, input: &CreateBook) -> Result<BookDetails> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(CoreError::validation("name is required"));
    }
    let slug = unique_slug(name, |candidate| async move {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM books WHERE slug = $1 AND deleted_at IS NULL)",
        )
        .bind(candidate)
        .fetch_one(db)
        .await?;
        Ok(exists)
    })
    .await?;

    let book = sqlx::query_as::<_, Book>(
        "INSERT INTO books (name, slug, description, created_by, updated_by) VALUES ($1, $2, $3, $4, $4) RETURNING *",
    )
    .bind(name)
    .bind(&slug)
    .bind(input.description.trim())
    .bind(user_id)
    .fetch_one(db)
    .await?;
    if !input.tags.is_empty() {
        tags::set_for(db, "book", book.id, &input.tags).await?;
    }
    get(db, book.id).await
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateBook {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<Tag>>,
}

pub async fn update(db: &PgPool, user_id: i64, id: i64, input: &UpdateBook) -> Result<BookDetails> {
    let current = fetch(db, id).await?;
    let name = input.name.clone().unwrap_or(current.name);
    let description = input.description.clone().unwrap_or(current.description);
    sqlx::query(
        "UPDATE books SET name = $2, description = $3, updated_by = $4, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(name.trim())
    .bind(description.trim())
    .bind(user_id)
    .execute(db)
    .await?;
    if let Some(t) = &input.tags {
        tags::set_for(db, "book", id, t).await?;
    }
    get(db, id).await
}

/// Soft-delete a book together with its chapters and pages. All rows share
/// the same transaction timestamp so a restore can bring them back together.
pub async fn delete(db: &PgPool, user_id: i64, id: i64) -> Result<()> {
    let book = fetch(db, id).await?;
    let mut tx = db.begin().await?;
    let res = sqlx::query("UPDATE books SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    sqlx::query("UPDATE chapters SET deleted_at = now() WHERE book_id = $1 AND deleted_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE pages SET deleted_at = now() WHERE book_id = $1 AND deleted_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    super::recycle::record(&mut tx, "book", id, &book.name, Some(user_id)).await?;
    tx.commit().await?;
    Ok(())
}
