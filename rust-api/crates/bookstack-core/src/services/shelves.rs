use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::tags;
use crate::models::{Book, ListParams, Paginated, Shelf, Tag};
use crate::slug::unique_slug;
use crate::{CoreError, Result};

#[derive(Debug, Serialize)]
pub struct ShelfDetails {
    #[serde(flatten)]
    pub shelf: Shelf,
    pub books: Vec<Book>,
    pub tags: Vec<Tag>,
}

pub async fn list(db: &PgPool, params: &ListParams) -> Result<Paginated<Shelf>> {
    let order = params.sort_sql(&["name", "id", "created_at", "updated_at"]);
    let data = sqlx::query_as::<_, Shelf>(&format!(
        "SELECT * FROM shelves WHERE deleted_at IS NULL ORDER BY {order} LIMIT $1 OFFSET $2"
    ))
    .bind(params.limit())
    .bind(params.offset())
    .fetch_all(db)
    .await?;
    let (total,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM shelves WHERE deleted_at IS NULL")
            .fetch_one(db)
            .await?;
    Ok(Paginated { data, total })
}

async fn fetch(db: &PgPool, id: i64) -> Result<Shelf> {
    sqlx::query_as::<_, Shelf>("SELECT * FROM shelves WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(CoreError::NotFound)
}

pub async fn get(db: &PgPool, id: i64) -> Result<ShelfDetails> {
    let shelf = fetch(db, id).await?;
    let books = sqlx::query_as::<_, Book>(
        r#"SELECT b.* FROM books b
           JOIN shelf_books sb ON sb.book_id = b.id
           WHERE sb.shelf_id = $1 AND b.deleted_at IS NULL
           ORDER BY sb."order", b.id"#,
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    let tags = tags::get_for(db, "shelf", id).await?;
    Ok(ShelfDetails { shelf, books, tags })
}

pub async fn get_by_slug(db: &PgPool, slug: &str) -> Result<ShelfDetails> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM shelves WHERE slug = $1 AND deleted_at IS NULL")
            .bind(slug)
            .fetch_optional(db)
            .await?;
    match row {
        Some((id,)) => get(db, id).await,
        None => Err(CoreError::NotFound),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateShelf {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub books: Vec<i64>,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

pub async fn create(db: &PgPool, user_id: i64, input: &CreateShelf) -> Result<ShelfDetails> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(CoreError::validation("name is required"));
    }
    let slug = unique_slug(name, |candidate| async move {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM shelves WHERE slug = $1 AND deleted_at IS NULL)",
        )
        .bind(candidate)
        .fetch_one(db)
        .await?;
        Ok(exists)
    })
    .await?;

    let shelf = sqlx::query_as::<_, Shelf>(
        "INSERT INTO shelves (name, slug, description, created_by, updated_by) VALUES ($1, $2, $3, $4, $4) RETURNING *",
    )
    .bind(name)
    .bind(&slug)
    .bind(input.description.trim())
    .bind(user_id)
    .fetch_one(db)
    .await?;

    set_books(db, shelf.id, &input.books).await?;
    if !input.tags.is_empty() {
        tags::set_for(db, "shelf", shelf.id, &input.tags).await?;
    }
    get(db, shelf.id).await
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateShelf {
    pub name: Option<String>,
    pub description: Option<String>,
    pub books: Option<Vec<i64>>,
    pub tags: Option<Vec<Tag>>,
}

pub async fn update(db: &PgPool, user_id: i64, id: i64, input: &UpdateShelf) -> Result<ShelfDetails> {
    let current = fetch(db, id).await?;
    let name = input.name.clone().unwrap_or(current.name);
    let description = input.description.clone().unwrap_or(current.description);
    sqlx::query(
        "UPDATE shelves SET name = $2, description = $3, updated_by = $4, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(name.trim())
    .bind(description.trim())
    .bind(user_id)
    .execute(db)
    .await?;
    if let Some(book_ids) = &input.books {
        set_books(db, id, book_ids).await?;
    }
    if let Some(t) = &input.tags {
        tags::set_for(db, "shelf", id, t).await?;
    }
    get(db, id).await
}

pub async fn set_books(db: &PgPool, shelf_id: i64, book_ids: &[i64]) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM shelf_books WHERE shelf_id = $1")
        .bind(shelf_id)
        .execute(&mut *tx)
        .await?;
    for (i, book_id) in book_ids.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO shelf_books (shelf_id, book_id, "order")
               SELECT $1, id, $3 FROM books WHERE id = $2 AND deleted_at IS NULL
               ON CONFLICT DO NOTHING"#,
        )
        .bind(shelf_id)
        .bind(book_id)
        .bind(i as i32)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete(db: &PgPool, user_id: i64, id: i64) -> Result<()> {
    let shelf = fetch(db, id).await?;
    let mut tx = db.begin().await?;
    let res = sqlx::query(
        "UPDATE shelves SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    super::recycle::record(&mut tx, "shelf", id, &shelf.name, Some(user_id)).await?;
    tx.commit().await?;
    Ok(())
}

/// Add a book to a shelf (appended at the end); no-op if already present.
pub async fn add_book(db: &PgPool, shelf_id: i64, book_id: i64) -> Result<()> {
    fetch(db, shelf_id).await?;
    super::books::fetch(db, book_id).await?;
    sqlx::query(
        r#"INSERT INTO shelf_books (shelf_id, book_id, "order")
           VALUES ($1, $2, (SELECT coalesce(max("order"), -1) + 1 FROM shelf_books WHERE shelf_id = $1))
           ON CONFLICT DO NOTHING"#,
    )
    .bind(shelf_id)
    .bind(book_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove_book(db: &PgPool, shelf_id: i64, book_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM shelf_books WHERE shelf_id = $1 AND book_id = $2")
        .bind(shelf_id)
        .bind(book_id)
        .execute(db)
        .await?;
    Ok(())
}
