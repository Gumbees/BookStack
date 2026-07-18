use sqlx::PgPool;

use crate::markdown::escape_html;
use crate::models::{Paginated, SearchResult};
use crate::Result;

pub const ALL_TYPES: [&str; 4] = ["page", "book", "chapter", "shelf"];

// Sentinel markers emitted by ts_headline, replaced with <mark> after the
// whole preview has been HTML-escaped (so user content can never inject HTML).
const HEADLINE_OPTS: &str =
    "'StartSel=@@BSHL@@, StopSel=@@BSHLE@@, MaxWords=35, MinWords=10, MaxFragments=2, FragmentDelimiter=\" … \"'";

/// Parsed BookStack-style search operators.
/// Supported subset: `{type:page|chapter}`, `[tag]`, `[tag=value]`,
/// `{in_name:term}`, `{created_by:me}`. Quoted phrases pass straight through
/// to `websearch_to_tsquery`, which handles them natively.
#[derive(Debug, Default)]
struct ParsedQuery {
    text: String,
    types: Vec<String>,
    tag_name: Option<String>,
    tag_value: Option<String>,
    in_name: Option<String>,
    created_by_me: bool,
}

fn parse_query(raw: &str) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    let mut text = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                let token: String = chars.by_ref().take_while(|c| *c != '}').collect();
                let (key, value) = token.split_once(':').unwrap_or((token.as_str(), ""));
                match key.trim().to_lowercase().as_str() {
                    "type" => {
                        parsed.types = value
                            .split('|')
                            .map(|t| t.trim().to_lowercase())
                            .filter(|t| !t.is_empty())
                            .collect();
                    }
                    "in_name" => parsed.in_name = Some(value.trim().to_string()),
                    "created_by" => parsed.created_by_me = value.trim() == "me",
                    _ => {} // unknown operators ignored gracefully
                }
            }
            '[' => {
                let token: String = chars.by_ref().take_while(|c| *c != ']').collect();
                let (name, value) = token.split_once('=').unwrap_or((token.as_str(), ""));
                if !name.trim().is_empty() {
                    parsed.tag_name = Some(name.trim().to_string());
                    if !value.trim().is_empty() {
                        parsed.tag_value = Some(value.trim().to_string());
                    }
                }
            }
            _ => text.push(ch),
        }
    }
    parsed.text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    parsed
}

/// Per-entity filter fragment shared by every subquery. Fixed bind positions:
/// $1 query text, $2 tag name, $3 tag value, $4 in_name, $5 created_by.
fn extra_filters(alias: &str, entity: &str) -> String {
    format!(
        " AND ($2::text IS NULL OR EXISTS (
              SELECT 1 FROM tags t
              WHERE t.entity_type = '{entity}' AND t.entity_id = {alias}.id
                AND lower(t.name) = lower($2)
                AND ($3::text IS NULL OR lower(t.value) = lower($3))))
          AND ($4::text IS NULL OR {alias}.name ILIKE '%' || $4 || '%')
          AND ($5::bigint IS NULL OR {alias}.created_by = $5)"
    )
}

fn subquery(entity: &str) -> String {
    let extra = match entity {
        "page" => extra_filters("p", "page"),
        "book" => extra_filters("b", "book"),
        "chapter" => extra_filters("c", "chapter"),
        _ => extra_filters("s", "shelf"),
    };
    match entity {
        "page" => format!(
            "SELECT 'page'::text AS entity_type, p.id AS id, p.name AS name, p.slug AS slug,
                    b.slug AS book_slug, ts_rank(p.search_vector, q) AS rank,
                    ts_headline('english', left(p.name || ' ' || p.markdown, 20000), q, {HEADLINE_OPTS}) AS preview
             FROM pages p
             JOIN books b ON b.id = p.book_id AND b.deleted_at IS NULL
             CROSS JOIN websearch_to_tsquery('english', $1) q
             WHERE p.deleted_at IS NULL AND p.draft = false
               AND ($1 = '' OR p.search_vector @@ q){extra}"
        ),
        "book" => format!(
            "SELECT 'book'::text, b.id, b.name, b.slug, NULL::text, ts_rank(b.search_vector, q),
                    ts_headline('english', b.name || ' ' || b.description, q, {HEADLINE_OPTS})
             FROM books b
             CROSS JOIN websearch_to_tsquery('english', $1) q
             WHERE b.deleted_at IS NULL AND ($1 = '' OR b.search_vector @@ q){extra}"
        ),
        "chapter" => format!(
            "SELECT 'chapter'::text, c.id, c.name, c.slug, b.slug, ts_rank(c.search_vector, q),
                    ts_headline('english', c.name || ' ' || c.description, q, {HEADLINE_OPTS})
             FROM chapters c
             JOIN books b ON b.id = c.book_id AND b.deleted_at IS NULL
             CROSS JOIN websearch_to_tsquery('english', $1) q
             WHERE c.deleted_at IS NULL AND ($1 = '' OR c.search_vector @@ q){extra}"
        ),
        _ => format!(
            "SELECT 'shelf'::text, s.id, s.name, s.slug, NULL::text, ts_rank(s.search_vector, q),
                    ts_headline('english', s.name || ' ' || s.description, q, {HEADLINE_OPTS})
             FROM shelves s
             CROSS JOIN websearch_to_tsquery('english', $1) q
             WHERE s.deleted_at IS NULL AND ($1 = '' OR s.search_vector @@ q){extra}"
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
    raw_query: &str,
    type_filter: &[String],
    count: i64,
    offset: i64,
    user_id: Option<i64>,
) -> Result<Paginated<SearchResult>> {
    let parsed = parse_query(raw_query);
    // Operator-only queries (e.g. just a tag filter) are allowed; a fully
    // empty query is not.
    let has_filter = parsed.tag_name.is_some() || parsed.in_name.is_some() || parsed.created_by_me;
    if parsed.text.is_empty() && !has_filter {
        return Ok(Paginated { data: vec![], total: 0 });
    }

    let mut selected: Vec<&str> = type_filter
        .iter()
        .chain(parsed.types.iter())
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
    let created_by = if parsed.created_by_me { user_id } else { None };

    let rows: Vec<(String, i64, String, String, Option<String>, f32, String)> = sqlx::query_as(
        &format!("SELECT * FROM ({union}) results ORDER BY rank DESC, entity_type, id LIMIT $6 OFFSET $7"),
    )
    .bind(&parsed.text)
    .bind(&parsed.tag_name)
    .bind(&parsed.tag_value)
    .bind(&parsed.in_name)
    .bind(created_by)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await?;

    let (total,): (i64,) = sqlx::query_as(&format!("SELECT count(*) FROM ({union}) results"))
        .bind(&parsed.text)
        .bind(&parsed.tag_name)
        .bind(&parsed.tag_value)
        .bind(&parsed.in_name)
        .bind(created_by)
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

#[cfg(test)]
mod tests {
    use super::parse_query;

    #[test]
    fn parses_operators() {
        let p = parse_query("deploy {type:page|book} [env=prod] {in_name:guide} {created_by:me}");
        assert_eq!(p.text, "deploy");
        assert_eq!(p.types, vec!["page", "book"]);
        assert_eq!(p.tag_name.as_deref(), Some("env"));
        assert_eq!(p.tag_value.as_deref(), Some("prod"));
        assert_eq!(p.in_name.as_deref(), Some("guide"));
        assert!(p.created_by_me);
    }

    #[test]
    fn plain_query_untouched() {
        let p = parse_query("hello \"exact phrase\" world");
        assert_eq!(p.text, "hello \"exact phrase\" world");
        assert!(p.types.is_empty());
        assert!(p.tag_name.is_none());
    }

    #[test]
    fn bare_tag() {
        let p = parse_query("[urgent]");
        assert_eq!(p.text, "");
        assert_eq!(p.tag_name.as_deref(), Some("urgent"));
        assert!(p.tag_value.is_none());
    }
}
