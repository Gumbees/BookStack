use sqlx::PgPool;

use crate::markdown::escape_html;
use crate::models::{Paginated, SearchResult};
use crate::Result;

pub const ALL_TYPES: [&str; 4] = ["page", "book", "chapter", "shelf"];

// Sentinel markers emitted by ts_headline, replaced with <mark> after the
// whole preview has been HTML-escaped (so user content can never inject HTML).
const HEADLINE_OPTS: &str =
    "'StartSel=@@BSHL@@, StopSel=@@BSHLE@@, MaxWords=35, MinWords=10, MaxFragments=2, FragmentDelimiter=\" … \"'";

fn subquery(entity: &str) -> String {
    match entity {
        "page" => format!(
            "SELECT 'page'::text AS entity_type, p.id AS id, p.name AS name, p.slug AS slug,
                    b.slug AS book_slug, ts_rank(p.search_vector, q) AS rank,
                    ts_headline('english', left(p.name || ' ' || p.markdown, 20000), q, {HEADLINE_OPTS}) AS preview
             FROM pages p
             JOIN books b ON b.id = p.book_id AND b.deleted_at IS NULL
             CROSS JOIN websearch_to_tsquery('english', $1) q
             WHERE p.deleted_at IS NULL AND p.draft = false AND p.search_vector @@ q"
        ),
        "book" => format!(
            "SELECT 'book'::text, b.id, b.name, b.slug, NULL::text, ts_rank(b.search_vector, q),
                    ts_headline('english', b.name || ' ' || b.description, q, {HEADLINE_OPTS})
             FROM books b
             CROSS JOIN websearch_to_tsquery('english', $1) q
             WHERE b.deleted_at IS NULL AND b.search_vector @@ q"
        ),
        "chapter" => format!(
            "SELECT 'chapter'::text, c.id, c.name, c.slug, b.slug, ts_rank(c.search_vector, q),
                    ts_headline('english', c.name || ' ' || c.description, q, {HEADLINE_OPTS})
             FROM chapters c
             JOIN books b ON b.id = c.book_id AND b.deleted_at IS NULL
             CROSS JOIN websearch_to_tsquery('english', $1) q
             WHERE c.deleted_at IS NULL AND c.search_vector @@ q"
        ),
        _ => format!(
            "SELECT 'shelf'::text, s.id, s.name, s.slug, NULL::text, ts_rank(s.search_vector, q),
                    ts_headline('english', s.name || ' ' || s.description, q, {HEADLINE_OPTS})
             FROM shelves s
             CROSS JOIN websearch_to_tsquery('english', $1) q
             WHERE s.deleted_at IS NULL AND s.search_vector @@ q"
        ),
    }
}

fn finish_preview(raw: &str) -> String {
    escape_html(raw)
        .replace("@@BSHL@@", "<mark>")
        .replace("@@BSHLE@@", "</mark>")
}

pub async fn search(
    db: &PgPool,
    query_text: &str,
    types: &[String],
    count: i64,
    offset: i64,
) -> Result<Paginated<SearchResult>> {
    let query_text = query_text.trim();
    if query_text.is_empty() {
        return Ok(Paginated { data: vec![], total: 0 });
    }
    let mut selected: Vec<&str> = types
        .iter()
        .map(|t| if t == "bookshelf" { "shelf" } else { t.as_str() })
        .filter(|t| ALL_TYPES.contains(t))
        .collect();
    if selected.is_empty() {
        selected = ALL_TYPES.to_vec();
    }
    selected.dedup();

    let union = selected.iter().map(|t| subquery(t)).collect::<Vec<_>>().join(" UNION ALL ");
    let limit = count.clamp(1, 100);
    let offset = offset.max(0);

    let rows: Vec<(String, i64, String, String, Option<String>, f32, String)> = sqlx::query_as(
        &format!("SELECT * FROM ({union}) results ORDER BY rank DESC, entity_type, id LIMIT $2 OFFSET $3"),
    )
    .bind(query_text)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await?;

    let (total,): (i64,) =
        sqlx::query_as(&format!("SELECT count(*) FROM ({union}) results"))
            .bind(query_text)
            .fetch_one(db)
            .await?;

    let data = rows
        .into_iter()
        .map(|(entity_type, id, name, slug, book_slug, rank, preview)| SearchResult {
            entity_type,
            id,
            name,
            slug,
            book_slug,
            rank,
            preview: finish_preview(&preview),
        })
        .collect();
    Ok(Paginated { data, total })
}
