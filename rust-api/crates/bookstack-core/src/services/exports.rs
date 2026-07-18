//! Content exports (markdown / plaintext / html), matching the
//! `export_page` / `export_chapter` / `export_book` bookstack-mcp tools.

use sqlx::PgPool;

use super::{books, chapters, pages};
use crate::markdown;
use crate::models::PAGE_COLS;
use crate::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Plaintext,
    Html,
}

impl ExportFormat {
    pub fn parse(s: &str) -> Result<ExportFormat> {
        match s {
            "markdown" | "md" => Ok(ExportFormat::Markdown),
            "plaintext" | "text" | "txt" => Ok(ExportFormat::Plaintext),
            "html" => Ok(ExportFormat::Html),
            other => Err(CoreError::validation(format!(
                "unsupported export format '{other}' (expected markdown, plaintext, or html)"
            ))),
        }
    }
}

fn render_one(name: &str, markdown_body: &str, html_body: &str, format: ExportFormat) -> String {
    match format {
        ExportFormat::Markdown => format!("# {name}\n\n{markdown_body}"),
        ExportFormat::Plaintext => format!("{name}\n\n{}", markdown::to_plaintext(markdown_body)),
        ExportFormat::Html => format!("<h1>{}</h1>\n{html_body}", markdown::escape_html(name)),
    }
}

pub async fn export_page(db: &PgPool, page_id: i64, format: ExportFormat) -> Result<String> {
    let page = pages::fetch(db, page_id).await?;
    Ok(render_one(&page.name, &page.markdown, &page.html, format))
}

pub async fn export_chapter(db: &PgPool, chapter_id: i64, format: ExportFormat) -> Result<String> {
    let chapter = chapters::fetch(db, chapter_id).await?;
    let chapter_pages = sqlx::query_as::<_, crate::models::Page>(&format!(
        "SELECT {PAGE_COLS} FROM pages WHERE chapter_id = $1 AND deleted_at IS NULL ORDER BY priority, id"
    ))
    .bind(chapter_id)
    .fetch_all(db)
    .await?;

    let mut out = match format {
        ExportFormat::Markdown => format!("# {}\n\n{}\n", chapter.name, chapter.description),
        ExportFormat::Plaintext => format!("{}\n\n{}\n", chapter.name, chapter.description),
        ExportFormat::Html => format!(
            "<h1>{}</h1>\n<p>{}</p>\n",
            markdown::escape_html(&chapter.name),
            markdown::escape_html(&chapter.description)
        ),
    };
    for page in chapter_pages {
        out.push('\n');
        out.push_str(&render_one(&page.name, &page.markdown, &page.html, format));
        out.push('\n');
    }
    Ok(out)
}

pub async fn export_book(db: &PgPool, book_id: i64, format: ExportFormat) -> Result<String> {
    let details = books::get(db, book_id).await?;
    let mut out = match format {
        ExportFormat::Markdown => format!("# {}\n\n{}\n", details.book.name, details.book.description),
        ExportFormat::Plaintext => format!("{}\n\n{}\n", details.book.name, details.book.description),
        ExportFormat::Html => format!(
            "<h1>{}</h1>\n<p>{}</p>\n",
            markdown::escape_html(&details.book.name),
            markdown::escape_html(&details.book.description)
        ),
    };
    for item in &details.contents {
        match item {
            crate::models::ContentItem::Chapter { chapter, .. } => {
                out.push('\n');
                out.push_str(&export_chapter(db, chapter.id, format).await?);
            }
            crate::models::ContentItem::Page { page } => {
                out.push('\n');
                out.push_str(&export_page(db, page.id, format).await?);
                out.push('\n');
            }
        }
    }
    Ok(out)
}
