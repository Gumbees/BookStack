//! MCP tool definitions and dispatch.

use serde_json::{json, Value};

use bookstack_core::models::{AuthUser, ListParams, Tag};
use bookstack_core::services::{books, chapters, pages, search, shelves, system};
use bookstack_core::CoreError;

use crate::McpServer;

pub enum ToolError {
    UnknownTool,
    Failed(String),
}

impl From<CoreError> for ToolError {
    fn from(err: CoreError) -> Self {
        ToolError::Failed(match err {
            CoreError::NotFound => "not found".to_string(),
            CoreError::Validation(msg) => msg,
            CoreError::Unauthorized => "unauthorized".to_string(),
            CoreError::Forbidden => "forbidden: insufficient permissions".to_string(),
            other => format!("internal error: {other}"),
        })
    }
}

type ToolResult = Result<Value, ToolError>;

fn req_str(args: &Value, key: &str) -> Result<String, ToolError> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::Failed(format!("missing required argument: {key}")))
}

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

fn req_i64(args: &Value, key: &str) -> Result<i64, ToolError> {
    args.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| ToolError::Failed(format!("missing required argument: {key}")))
}

fn opt_i64(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(Value::as_i64)
}

fn list_params(args: &Value) -> ListParams {
    ListParams {
        count: opt_i64(args, "count"),
        offset: opt_i64(args, "offset"),
        sort: opt_str(args, "sort"),
        order: opt_str(args, "order"),
    }
}

fn tags_arg(args: &Value) -> Vec<Tag> {
    args.get("tags")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

fn require_edit(user: &AuthUser) -> Result<(), ToolError> {
    if user.role.can_edit() {
        Ok(())
    } else {
        Err(CoreError::Forbidden.into())
    }
}

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

fn id_prop(desc: &str) -> Value {
    json!({ "type": "integer", "description": desc })
}

fn paging_props() -> Value {
    json!({
        "count": { "type": "integer", "description": "Max results to return (default 100)" },
        "offset": { "type": "integer", "description": "Results to skip" },
    })
}

pub fn definitions() -> Vec<Value> {
    let mut defs = Vec::new();
    let mut tool = |name: &str, description: &str, input_schema: Value| {
        defs.push(json!({ "name": name, "description": description, "inputSchema": input_schema }));
    };

    tool(
        "search_content",
        "Search across shelves, books, chapters and pages using full-text search. Returns ranked results with highlighted previews.",
        schema(
            json!({
                "query": { "type": "string", "description": "Search query (supports quoted phrases and -exclusions)" },
                "types": { "type": "array", "items": { "type": "string", "enum": ["page", "book", "chapter", "shelf"] }, "description": "Restrict to these entity types" },
                "count": { "type": "integer" },
                "offset": { "type": "integer" },
            }),
            &["query"],
        ),
    );
    tool("list_shelves", "List all shelves.", schema(paging_props(), &[]));
    tool("get_shelf", "Get a shelf with its books and tags.", schema(json!({ "id": id_prop("Shelf ID") }), &["id"]));
    tool(
        "create_shelf",
        "Create a new shelf. Requires editor permissions.",
        schema(
            json!({
                "name": { "type": "string" },
                "description": { "type": "string" },
                "books": { "type": "array", "items": { "type": "integer" }, "description": "Ordered book IDs to place on the shelf" },
            }),
            &["name"],
        ),
    );
    tool("list_books", "List all books.", schema(paging_props(), &[]));
    tool("get_book", "Get a book including its full contents tree (chapters and pages) and tags.", schema(json!({ "id": id_prop("Book ID") }), &["id"]));
    tool(
        "create_book",
        "Create a new book. Requires editor permissions.",
        schema(json!({ "name": { "type": "string" }, "description": { "type": "string" } }), &["name"]),
    );
    tool(
        "update_book",
        "Update a book's name or description. Requires editor permissions.",
        schema(
            json!({ "id": id_prop("Book ID"), "name": { "type": "string" }, "description": { "type": "string" } }),
            &["id"],
        ),
    );
    tool("delete_book", "Delete a book (and its chapters/pages). Requires editor permissions.", schema(json!({ "id": id_prop("Book ID") }), &["id"]));
    tool(
        "list_chapters",
        "List chapters, optionally filtered by book.",
        schema(
            json!({ "book_id": id_prop("Only chapters of this book"), "count": { "type": "integer" }, "offset": { "type": "integer" } }),
            &[],
        ),
    );
    tool("get_chapter", "Get a chapter with its pages and tags.", schema(json!({ "id": id_prop("Chapter ID") }), &["id"]));
    tool(
        "create_chapter",
        "Create a chapter within a book. Requires editor permissions.",
        schema(
            json!({ "book_id": id_prop("Parent book ID"), "name": { "type": "string" }, "description": { "type": "string" } }),
            &["book_id", "name"],
        ),
    );
    tool(
        "list_pages",
        "List pages, optionally filtered by book or chapter.",
        schema(
            json!({
                "book_id": id_prop("Only pages of this book"),
                "chapter_id": id_prop("Only pages of this chapter"),
                "count": { "type": "integer" },
                "offset": { "type": "integer" },
            }),
            &[],
        ),
    );
    tool("get_page", "Get a page including its markdown content and tags.", schema(json!({ "id": id_prop("Page ID") }), &["id"]));
    tool(
        "create_page",
        "Create a page in a book (optionally inside a chapter) with markdown content. Requires editor permissions.",
        schema(
            json!({
                "book_id": id_prop("Parent book ID"),
                "chapter_id": id_prop("Optional parent chapter ID"),
                "name": { "type": "string" },
                "markdown": { "type": "string", "description": "Page content in Markdown" },
            }),
            &["book_id", "name"],
        ),
    );
    tool(
        "update_page",
        "Update a page's name and/or markdown content (replaces content). Requires editor permissions.",
        schema(
            json!({
                "id": id_prop("Page ID"),
                "name": { "type": "string" },
                "markdown": { "type": "string" },
                "summary": { "type": "string", "description": "Change summary stored on the revision" },
            }),
            &["id"],
        ),
    );
    tool(
        "append_to_page",
        "Append markdown to the end of an existing page. Requires editor permissions.",
        schema(
            json!({ "id": id_prop("Page ID"), "markdown": { "type": "string", "description": "Markdown to append" } }),
            &["id", "markdown"],
        ),
    );
    tool(
        "move_page",
        "Move a page to a different book and/or chapter. Requires editor permissions.",
        schema(
            json!({ "id": id_prop("Page ID"), "book_id": id_prop("Target book ID"), "chapter_id": id_prop("Optional target chapter ID") }),
            &["id", "book_id"],
        ),
    );
    tool("delete_page", "Delete a page. Requires editor permissions.", schema(json!({ "id": id_prop("Page ID") }), &["id"]));
    tool(
        "list_page_revisions",
        "List the revision history of a page.",
        schema(json!({ "id": id_prop("Page ID"), "count": { "type": "integer" }, "offset": { "type": "integer" } }), &["id"]),
    );
    tool("get_system_info", "Get instance name, version and content counts.", schema(json!({}), &[]));

    defs
}

pub async fn call(server: &McpServer, user: &AuthUser, name: &str, args: &Value) -> ToolResult {
    let db = &server.core.db;
    match name {
        "search_content" => {
            let query = req_str(args, "query")?;
            let types: Vec<String> = args
                .get("types")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default();
            let count = opt_i64(args, "count").unwrap_or(20);
            let offset = opt_i64(args, "offset").unwrap_or(0);
            let results = search::search(db, &query, &types, count, offset).await?;
            Ok(serde_json::to_value(results).unwrap())
        }
        "list_shelves" => Ok(serde_json::to_value(shelves::list(db, &list_params(args)).await?).unwrap()),
        "get_shelf" => Ok(serde_json::to_value(shelves::get(db, req_i64(args, "id")?).await?).unwrap()),
        "create_shelf" => {
            require_edit(user)?;
            let input = shelves::CreateShelf {
                name: req_str(args, "name")?,
                description: opt_str(args, "description").unwrap_or_default(),
                books: args
                    .get("books")
                    .and_then(|value| serde_json::from_value(value.clone()).ok())
                    .unwrap_or_default(),
                tags: tags_arg(args),
            };
            Ok(serde_json::to_value(shelves::create(db, user.id, &input).await?).unwrap())
        }
        "list_books" => Ok(serde_json::to_value(books::list(db, &list_params(args)).await?).unwrap()),
        "get_book" => Ok(serde_json::to_value(books::get(db, req_i64(args, "id")?).await?).unwrap()),
        "create_book" => {
            require_edit(user)?;
            let input = books::CreateBook {
                name: req_str(args, "name")?,
                description: opt_str(args, "description").unwrap_or_default(),
                tags: tags_arg(args),
            };
            Ok(serde_json::to_value(books::create(db, user.id, &input).await?).unwrap())
        }
        "update_book" => {
            require_edit(user)?;
            let input = books::UpdateBook {
                name: opt_str(args, "name"),
                description: opt_str(args, "description"),
                tags: None,
            };
            Ok(serde_json::to_value(books::update(db, user.id, req_i64(args, "id")?, &input).await?).unwrap())
        }
        "delete_book" => {
            require_edit(user)?;
            books::delete(db, req_i64(args, "id")?).await?;
            Ok(json!({ "deleted": true }))
        }
        "list_chapters" => {
            let result = chapters::list(db, &list_params(args), opt_i64(args, "book_id")).await?;
            Ok(serde_json::to_value(result).unwrap())
        }
        "get_chapter" => Ok(serde_json::to_value(chapters::get(db, req_i64(args, "id")?).await?).unwrap()),
        "create_chapter" => {
            require_edit(user)?;
            let input = chapters::CreateChapter {
                book_id: req_i64(args, "book_id")?,
                name: req_str(args, "name")?,
                description: opt_str(args, "description").unwrap_or_default(),
                tags: tags_arg(args),
            };
            Ok(serde_json::to_value(chapters::create(db, user.id, &input).await?).unwrap())
        }
        "list_pages" => {
            let result = pages::list(
                db,
                &list_params(args),
                opt_i64(args, "book_id"),
                opt_i64(args, "chapter_id"),
            )
            .await?;
            Ok(serde_json::to_value(result).unwrap())
        }
        "get_page" => Ok(serde_json::to_value(pages::get(db, req_i64(args, "id")?).await?).unwrap()),
        "create_page" => {
            require_edit(user)?;
            let input = pages::CreatePage {
                book_id: req_i64(args, "book_id")?,
                chapter_id: opt_i64(args, "chapter_id"),
                name: req_str(args, "name")?,
                markdown: opt_str(args, "markdown").unwrap_or_default(),
                draft: false,
                tags: tags_arg(args),
            };
            Ok(serde_json::to_value(pages::create(db, user.id, &input).await?).unwrap())
        }
        "update_page" => {
            require_edit(user)?;
            let id = req_i64(args, "id")?;
            let input = pages::UpdatePage {
                name: opt_str(args, "name"),
                markdown: opt_str(args, "markdown"),
                summary: opt_str(args, "summary").or(Some("Updated via MCP".to_string())),
                ..Default::default()
            };
            let (details, content_changed) = pages::update(db, user.id, id, &input).await?;
            if content_changed {
                server.collab.invalidate(id).await;
            }
            Ok(serde_json::to_value(details).unwrap())
        }
        "append_to_page" => {
            require_edit(user)?;
            let id = req_i64(args, "id")?;
            let extra = req_str(args, "markdown")?;
            let current = pages::fetch(db, id).await?;
            let mut markdown = current.markdown;
            if !markdown.is_empty() && !markdown.ends_with('\n') {
                markdown.push('\n');
            }
            if !markdown.is_empty() {
                markdown.push('\n');
            }
            markdown.push_str(&extra);
            let input = pages::UpdatePage {
                markdown: Some(markdown),
                summary: Some("Appended content via MCP".to_string()),
                ..Default::default()
            };
            let (details, content_changed) = pages::update(db, user.id, id, &input).await?;
            if content_changed {
                server.collab.invalidate(id).await;
            }
            Ok(serde_json::to_value(details).unwrap())
        }
        "move_page" => {
            require_edit(user)?;
            let result = pages::move_page(
                db,
                user.id,
                req_i64(args, "id")?,
                req_i64(args, "book_id")?,
                opt_i64(args, "chapter_id"),
            )
            .await?;
            Ok(serde_json::to_value(result).unwrap())
        }
        "delete_page" => {
            require_edit(user)?;
            let id = req_i64(args, "id")?;
            pages::delete(db, id).await?;
            server.collab.invalidate(id).await;
            Ok(json!({ "deleted": true }))
        }
        "list_page_revisions" => {
            let result = pages::revisions(db, req_i64(args, "id")?, &list_params(args)).await?;
            Ok(serde_json::to_value(result).unwrap())
        }
        "get_system_info" => Ok(serde_json::to_value(system::info(db).await?).unwrap()),
        _ => Err(ToolError::UnknownTool),
    }
}
