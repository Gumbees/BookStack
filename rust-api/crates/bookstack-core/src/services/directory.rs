//! Scoped, depth-limited content tree (the bookstack-mcp `directory` tool).
//! Reads straight from Postgres — no separate index needed here.

use serde::Serialize;
use sqlx::PgPool;

use crate::{CoreError, Result};

#[derive(Debug, Clone, Copy)]
pub enum Scope {
    All,
    Shelf(i64),
    Book(i64),
    Chapter(i64),
}

#[derive(Debug, Serialize)]
pub struct Node {
    pub kind: &'static str,
    pub id: i64,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

struct Raw {
    shelves: Vec<(i64, String, String)>,
    shelf_books: Vec<(i64, i64)>,
    books: Vec<(i64, String, String)>,
    chapters: Vec<(i64, i64, String, String)>,
    pages: Vec<(i64, i64, Option<i64>, String, String)>,
}

async fn load(db: &PgPool, org_id: i64) -> Result<Raw> {
    Ok(Raw {
        shelves: sqlx::query_as(
            "SELECT id, name, slug FROM shelves WHERE org_id = $1 AND deleted_at IS NULL ORDER BY name",
        )
        .bind(org_id)
        .fetch_all(db)
        .await?,
        shelf_books: sqlx::query_as(
            r#"SELECT sb.shelf_id, sb.book_id FROM shelf_books sb
               JOIN shelves s ON s.id = sb.shelf_id WHERE s.org_id = $1
               ORDER BY sb.shelf_id, sb."order""#,
        )
        .bind(org_id)
        .fetch_all(db)
        .await?,
        books: sqlx::query_as(
            "SELECT id, name, slug FROM books WHERE org_id = $1 AND deleted_at IS NULL ORDER BY name",
        )
        .bind(org_id)
        .fetch_all(db)
        .await?,
        chapters: sqlx::query_as(
            "SELECT id, book_id, name, slug FROM chapters WHERE org_id = $1 AND deleted_at IS NULL ORDER BY priority, id",
        )
        .bind(org_id)
        .fetch_all(db)
        .await?,
        pages: sqlx::query_as(
            "SELECT id, book_id, chapter_id, name, slug FROM pages WHERE org_id = $1 AND deleted_at IS NULL ORDER BY priority, id",
        )
        .bind(org_id)
        .fetch_all(db)
        .await?,
    })
}

fn page_nodes(raw: &Raw, book_id: i64, chapter_id: Option<i64>) -> Vec<Node> {
    raw.pages
        .iter()
        .filter(|(_, b, c, _, _)| *b == book_id && *c == chapter_id)
        .map(|(id, _, _, name, slug)| Node {
            kind: "page",
            id: *id,
            name: name.clone(),
            slug: slug.clone(),
            children: vec![],
        })
        .collect()
}

fn chapter_node(raw: &Raw, id: i64, book_id: i64, name: &str, slug: &str, depth_left: Option<u32>) -> Node {
    let children = match depth_left {
        Some(0) => vec![],
        _ => page_nodes(raw, book_id, Some(id)),
    };
    Node { kind: "chapter", id, name: name.to_string(), slug: slug.to_string(), children }
}

fn book_node(raw: &Raw, id: i64, name: &str, slug: &str, depth_left: Option<u32>) -> Node {
    let children = match depth_left {
        Some(0) => vec![],
        _ => {
            let next = depth_left.map(|d| d - 1);
            let mut children: Vec<Node> = raw
                .chapters
                .iter()
                .filter(|(_, b, _, _)| *b == id)
                .map(|(cid, b, cname, cslug)| chapter_node(raw, *cid, *b, cname, cslug, next))
                .collect();
            children.extend(page_nodes(raw, id, None));
            children
        }
    };
    Node { kind: "book", id, name: name.to_string(), slug: slug.to_string(), children }
}

fn shelf_node(raw: &Raw, id: i64, name: &str, slug: &str, depth_left: Option<u32>) -> Node {
    let children = match depth_left {
        Some(0) => vec![],
        _ => {
            let next = depth_left.map(|d| d - 1);
            raw.shelf_books
                .iter()
                .filter(|(shelf, _)| *shelf == id)
                .filter_map(|(_, book_id)| {
                    raw.books
                        .iter()
                        .find(|(bid, _, _)| bid == book_id)
                        .map(|(bid, bname, bslug)| book_node(raw, *bid, bname, bslug, next))
                })
                .collect()
        }
    };
    Node { kind: "shelf", id, name: name.to_string(), slug: slug.to_string(), children }
}

/// Build the tree. `depth` counts levels below the roots (0 = roots only).
pub async fn tree(db: &PgPool, org_id: i64, scope: Scope, depth: Option<u32>) -> Result<Vec<Node>> {
    let raw = load(db, org_id).await?;
    match scope {
        Scope::All => {
            let mut roots: Vec<Node> = raw
                .shelves
                .iter()
                .map(|(id, name, slug)| shelf_node(&raw, *id, name, slug, depth))
                .collect();
            // Books on no shelf appear as additional roots.
            let shelved: std::collections::HashSet<i64> =
                raw.shelf_books.iter().map(|(_, b)| *b).collect();
            roots.extend(
                raw.books
                    .iter()
                    .filter(|(id, _, _)| !shelved.contains(id))
                    .map(|(id, name, slug)| book_node(&raw, *id, name, slug, depth)),
            );
            Ok(roots)
        }
        Scope::Shelf(id) => {
            let (id, name, slug) = raw
                .shelves
                .iter()
                .find(|(sid, _, _)| *sid == id)
                .ok_or(CoreError::NotFound)?
                .clone();
            Ok(vec![shelf_node(&raw, id, &name, &slug, depth)])
        }
        Scope::Book(id) => {
            let (id, name, slug) = raw
                .books
                .iter()
                .find(|(bid, _, _)| *bid == id)
                .ok_or(CoreError::NotFound)?
                .clone();
            Ok(vec![book_node(&raw, id, &name, &slug, depth)])
        }
        Scope::Chapter(id) => {
            let (id, book_id, name, slug) = raw
                .chapters
                .iter()
                .find(|(cid, _, _, _)| *cid == id)
                .ok_or(CoreError::NotFound)?
                .clone();
            Ok(vec![chapter_node(&raw, id, book_id, &name, &slug, depth)])
        }
    }
}
