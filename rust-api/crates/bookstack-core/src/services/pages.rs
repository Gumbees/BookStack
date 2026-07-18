use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::{books, chapters, tags};
use crate::markdown;
use crate::models::{ListParams, Page, PageMeta, PageRevision, Paginated, Tag, PAGE_COLS};
use crate::slug::unique_slug;
use crate::{CoreError, Result};

#[derive(Debug, Serialize)]
pub struct PageDetails {
    #[serde(flatten)]
    pub page: Page,
    pub tags: Vec<Tag>,
    pub book_slug: String,
}

pub async fn list(
    db: &PgPool,
    params: &ListParams,
    book_id: Option<i64>,
    chapter_id: Option<i64>,
) -> Result<Paginated<PageMeta>> {
    let order = params.sort_sql(&["priority", "name", "id", "created_at", "updated_at"]);
    let mut filter = String::new();
    if book_id.is_some() {
        filter.push_str(" AND book_id = $3");
    }
    if chapter_id.is_some() {
        filter.push_str(if book_id.is_some() { " AND chapter_id = $4" } else { " AND chapter_id = $3" });
    }
    let sql = format!(
        "SELECT id, book_id, chapter_id, name, slug, priority, draft, revision_count, created_by, updated_by, created_at, updated_at
         FROM pages WHERE deleted_at IS NULL{filter} ORDER BY {order} LIMIT $1 OFFSET $2"
    );
    let mut query = sqlx::query_as::<_, PageMeta>(&sql).bind(params.limit()).bind(params.offset());
    if let Some(id) = book_id {
        query = query.bind(id);
    }
    if let Some(id) = chapter_id {
        query = query.bind(id);
    }
    let data = query.fetch_all(db).await?;

    let count_sql = format!(
        "SELECT count(*) FROM pages WHERE deleted_at IS NULL{}",
        filter.replace("$3", "$1").replace("$4", "$2")
    );
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
    if let Some(id) = book_id {
        count_query = count_query.bind(id);
    }
    if let Some(id) = chapter_id {
        count_query = count_query.bind(id);
    }
    let (total,) = count_query.fetch_one(db).await?;
    Ok(Paginated { data, total })
}

pub async fn fetch(db: &PgPool, id: i64) -> Result<Page> {
    sqlx::query_as::<_, Page>(&format!(
        "SELECT {PAGE_COLS} FROM pages WHERE id = $1 AND deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or(CoreError::NotFound)
}

async fn details(db: &PgPool, page: Page) -> Result<PageDetails> {
    let tags = tags::get_for(db, "page", page.id).await?;
    let (book_slug,): (String,) = sqlx::query_as("SELECT slug FROM books WHERE id = $1")
        .bind(page.book_id)
        .fetch_one(db)
        .await?;
    Ok(PageDetails { page, tags, book_slug })
}

pub async fn get(db: &PgPool, id: i64) -> Result<PageDetails> {
    let page = fetch(db, id).await?;
    details(db, page).await
}

pub async fn get_by_slugs(db: &PgPool, book_slug: &str, page_slug: &str) -> Result<PageDetails> {
    let cols: String = PAGE_COLS
        .split(", ")
        .map(|c| format!("p.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let page = sqlx::query_as::<_, Page>(&format!(
        "SELECT {cols} FROM pages p
         JOIN books b ON b.id = p.book_id
         WHERE b.slug = $1 AND p.slug = $2 AND p.deleted_at IS NULL AND b.deleted_at IS NULL"
    ))
    .bind(book_slug)
    .bind(page_slug)
    .fetch_optional(db)
    .await?
    .ok_or(CoreError::NotFound)?;
    details(db, page).await
}

async fn page_slug_for(db: &PgPool, book_id: i64, name: &str) -> Result<String> {
    unique_slug(name, |candidate| async move {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM pages WHERE book_id = $1 AND slug = $2 AND deleted_at IS NULL)",
        )
        .bind(book_id)
        .bind(candidate)
        .fetch_one(db)
        .await?;
        Ok(exists)
    })
    .await
}

async fn next_priority(db: &PgPool, book_id: i64, chapter_id: Option<i64>) -> Result<i32> {
    let (max,): (Option<i32>,) = match chapter_id {
        Some(cid) => {
            sqlx::query_as("SELECT max(priority) FROM pages WHERE chapter_id = $1 AND deleted_at IS NULL")
                .bind(cid)
                .fetch_one(db)
                .await?
        }
        None => {
            sqlx::query_as(
                "SELECT greatest(
                    (SELECT max(priority) FROM pages WHERE book_id = $1 AND chapter_id IS NULL AND deleted_at IS NULL),
                    (SELECT max(priority) FROM chapters WHERE book_id = $1 AND deleted_at IS NULL)
                )",
            )
            .bind(book_id)
            .fetch_one(db)
            .await?
        }
    };
    Ok(max.unwrap_or(0) + 1)
}

async fn add_revision(
    db: &PgPool,
    page_id: i64,
    user_id: Option<i64>,
    summary: &str,
) -> Result<i32> {
    let page = fetch(db, page_id).await?;
    let number = page.revision_count + 1;
    sqlx::query(
        "INSERT INTO page_revisions (page_id, revision_number, name, markdown, summary, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(page_id)
    .bind(number)
    .bind(&page.name)
    .bind(&page.markdown)
    .bind(summary)
    .bind(user_id)
    .execute(db)
    .await?;
    sqlx::query("UPDATE pages SET revision_count = $2 WHERE id = $1")
        .bind(page_id)
        .bind(number)
        .execute(db)
        .await?;
    Ok(number)
}

#[derive(Debug, Deserialize)]
pub struct CreatePage {
    pub book_id: i64,
    #[serde(default)]
    pub chapter_id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub markdown: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

pub async fn create(db: &PgPool, user_id: i64, input: &CreatePage) -> Result<PageDetails> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(CoreError::validation("name is required"));
    }
    books::fetch(db, input.book_id).await?;
    if let Some(cid) = input.chapter_id {
        let chapter = chapters::fetch(db, cid).await?;
        if chapter.book_id != input.book_id {
            return Err(CoreError::validation("chapter does not belong to the given book"));
        }
    }
    let slug = page_slug_for(db, input.book_id, name).await?;
    let priority = next_priority(db, input.book_id, input.chapter_id).await?;
    let html = markdown::render(&input.markdown);

    let page = sqlx::query_as::<_, Page>(&format!(
        "INSERT INTO pages (book_id, chapter_id, name, slug, markdown, html, priority, draft, created_by, updated_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9) RETURNING {PAGE_COLS}"
    ))
    .bind(input.book_id)
    .bind(input.chapter_id)
    .bind(name)
    .bind(&slug)
    .bind(&input.markdown)
    .bind(&html)
    .bind(priority)
    .bind(input.draft)
    .bind(user_id)
    .fetch_one(db)
    .await?;

    if !input.tags.is_empty() {
        tags::set_for(db, "page", page.id, &input.tags).await?;
    }
    add_revision(db, page.id, Some(user_id), "Initial version").await?;
    get(db, page.id).await
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdatePage {
    pub name: Option<String>,
    pub markdown: Option<String>,
    pub summary: Option<String>,
    pub priority: Option<i32>,
    pub draft: Option<bool>,
    pub tags: Option<Vec<Tag>>,
}

/// Update a page. Returns the updated page and whether the markdown content
/// changed (callers should invalidate any live collaboration room when true).
pub async fn update(
    db: &PgPool,
    user_id: i64,
    id: i64,
    input: &UpdatePage,
) -> Result<(PageDetails, bool)> {
    let current = fetch(db, id).await?;
    let name = input.name.clone().map(|n| n.trim().to_string()).unwrap_or(current.name.clone());
    if name.is_empty() {
        return Err(CoreError::validation("name cannot be empty"));
    }
    let markdown_body = input.markdown.clone().unwrap_or(current.markdown.clone());
    let content_changed = markdown_body != current.markdown;
    let name_changed = name != current.name;
    let priority = input.priority.unwrap_or(current.priority);
    let draft = input.draft.unwrap_or(current.draft);
    let html = if content_changed { markdown::render(&markdown_body) } else { current.html.clone() };

    // A REST/MCP content edit supersedes any persisted CRDT state: clear it so
    // the next collaborative session re-seeds from the new markdown.
    sqlx::query(
        "UPDATE pages SET name = $2, markdown = $3, html = $4, priority = $5, draft = $6,
                          updated_by = $7, updated_at = now(),
                          ydoc_state = CASE WHEN $8 THEN NULL ELSE ydoc_state END
         WHERE id = $1",
    )
    .bind(id)
    .bind(&name)
    .bind(&markdown_body)
    .bind(&html)
    .bind(priority)
    .bind(draft)
    .bind(user_id)
    .bind(content_changed)
    .execute(db)
    .await?;

    if let Some(t) = &input.tags {
        tags::set_for(db, "page", id, t).await?;
    }
    if content_changed || name_changed {
        let summary = input.summary.clone().unwrap_or_else(|| "Updated page".to_string());
        add_revision(db, id, Some(user_id), &summary).await?;
    }
    Ok((get(db, id).await?, content_changed))
}

pub async fn move_page(
    db: &PgPool,
    user_id: i64,
    id: i64,
    book_id: i64,
    chapter_id: Option<i64>,
) -> Result<PageDetails> {
    let current = fetch(db, id).await?;
    books::fetch(db, book_id).await?;
    if let Some(cid) = chapter_id {
        let chapter = chapters::fetch(db, cid).await?;
        if chapter.book_id != book_id {
            return Err(CoreError::validation("chapter does not belong to the given book"));
        }
    }
    // Re-slug when moving across books to keep (book, slug) unique.
    let slug = if book_id != current.book_id {
        page_slug_for(db, book_id, &current.name).await?
    } else {
        current.slug.clone()
    };
    let priority = next_priority(db, book_id, chapter_id).await?;
    sqlx::query(
        "UPDATE pages SET book_id = $2, chapter_id = $3, slug = $4, priority = $5, updated_by = $6, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(book_id)
    .bind(chapter_id)
    .bind(&slug)
    .bind(priority)
    .bind(user_id)
    .execute(db)
    .await?;
    get(db, id).await
}

pub async fn delete(db: &PgPool, id: i64) -> Result<()> {
    let res = sqlx::query("UPDATE pages SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

pub async fn revisions(db: &PgPool, page_id: i64, params: &ListParams) -> Result<Paginated<PageRevision>> {
    fetch(db, page_id).await?;
    let data = sqlx::query_as::<_, PageRevision>(
        "SELECT * FROM page_revisions WHERE page_id = $1 ORDER BY revision_number DESC LIMIT $2 OFFSET $3",
    )
    .bind(page_id)
    .bind(params.limit())
    .bind(params.offset())
    .fetch_all(db)
    .await?;
    let (total,): (i64,) = sqlx::query_as("SELECT count(*) FROM page_revisions WHERE page_id = $1")
        .bind(page_id)
        .fetch_one(db)
        .await?;
    Ok(Paginated { data, total })
}

pub async fn restore_revision(
    db: &PgPool,
    user_id: i64,
    page_id: i64,
    revision_number: i32,
) -> Result<PageDetails> {
    let revision = sqlx::query_as::<_, PageRevision>(
        "SELECT * FROM page_revisions WHERE page_id = $1 AND revision_number = $2",
    )
    .bind(page_id)
    .bind(revision_number)
    .fetch_optional(db)
    .await?
    .ok_or(CoreError::NotFound)?;

    let (details, _) = update(
        db,
        user_id,
        page_id,
        &UpdatePage {
            name: Some(revision.name.clone()),
            markdown: Some(revision.markdown.clone()),
            summary: Some(format!("Restored revision #{revision_number}")),
            ..Default::default()
        },
    )
    .await?;
    Ok(details)
}

/// Content + persisted CRDT state used to seed a collaboration room.
pub struct CollabSource {
    pub name: String,
    pub markdown: String,
    pub ydoc_state: Option<Vec<u8>>,
}

pub async fn collab_source(db: &PgPool, id: i64) -> Result<CollabSource> {
    let row: Option<(String, String, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT name, markdown, ydoc_state FROM pages WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    let (name, markdown, ydoc_state) = row.ok_or(CoreError::NotFound)?;
    Ok(CollabSource { name, markdown, ydoc_state })
}

/// Persist the flattened state of a live collaboration document.
/// Does not create a revision (see `snapshot_revision`).
pub async fn persist_collab(db: &PgPool, id: i64, markdown_body: &str, ydoc_state: &[u8]) -> Result<()> {
    let html = markdown::render(markdown_body);
    let res = sqlx::query(
        "UPDATE pages SET markdown = $2, html = $3, ydoc_state = $4, updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(markdown_body)
    .bind(&html)
    .bind(ydoc_state)
    .execute(db)
    .await?;
    if res.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Record a revision snapshot of the page's current stored content.
pub async fn snapshot_revision(
    db: &PgPool,
    id: i64,
    user_id: Option<i64>,
    summary: &str,
) -> Result<i32> {
    add_revision(db, id, user_id, summary).await
}
