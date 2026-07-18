//! MCP tool definitions and dispatch, ported from
//! `bees-roadhouse/bookstack-mcp` (crates/bsmcp-server/src/mcp.rs) and backed
//! directly by the core Postgres services.

use serde_json::{json, Value};

use bookstack_core::models::{AuthUser, ListParams};
use bookstack_core::services::{
    books, chapters, comments, directory, exports, pages, recycle, search, shelves, system, users,
};
use bookstack_core::CoreError;

use crate::McpServer;

type ToolResult = Result<String, String>;

// --- argument helpers (upstream conventions) ---

fn arg_str(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("{key} is required"))
}

fn arg_str_opt(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

fn arg_str_default(args: &Value, key: &str, default: &str) -> String {
    arg_str_opt(args, key).unwrap_or_else(|| default.to_string())
}

fn arg_i64_required(args: &Value, key: &str) -> Result<i64, String> {
    args.get(key)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| format!("{key} is required"))
}

fn arg_i64_opt(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

fn arg_i64(args: &Value, key: &str, default: i64) -> i64 {
    arg_i64_opt(args, key).unwrap_or(default)
}

fn arg_bool(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn arg_i64_array(args: &Value, key: &str) -> Vec<i64> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default()
}

fn list_params(args: &Value) -> ListParams {
    ListParams {
        count: arg_i64_opt(args, "count"),
        offset: arg_i64_opt(args, "offset"),
        sort: None,
        order: None,
    }
}

fn map_core(err: CoreError) -> String {
    match err {
        CoreError::NotFound => "not found".to_string(),
        CoreError::Validation(msg) => msg,
        CoreError::Unauthorized => "unauthorized".to_string(),
        CoreError::Forbidden => "forbidden: insufficient permissions".to_string(),
        other => format!("internal error: {other}"),
    }
}

fn require_edit(user: &AuthUser) -> Result<(), String> {
    if user.role.can_edit() {
        Ok(())
    } else {
        Err("forbidden: this operation requires the editor or admin role".to_string())
    }
}

/// Require a non-empty, meaningful description when creating shelves/books/
/// chapters (ported: descriptions are surfaced to every connecting client in
/// the structure listing, so placeholders actively degrade future routing).
fn require_description(args: &Value, kind: &str) -> Result<String, String> {
    let raw = args.get("description").and_then(|v| v.as_str()).unwrap_or("").trim();
    if raw.is_empty() {
        return Err(format!(
            "description is required when creating a {kind}. \
             Descriptions are surfaced to all clients that connect to this BookStack, \
             so they shape placement decisions for every future page created here. \
             Provide a 1-2 sentence description that answers (1) what kind of content lives in \
             this {kind}, and (2) what it's for. Avoid placeholders like 'TODO' or 'description'."
        ));
    }
    if raw.len() < 15 {
        return Err(format!(
            "description is too short ({} chars) — write a meaningful 1-2 sentence description \
             that tells future clients what content belongs in this {kind} and what it's for.",
            raw.len()
        ));
    }
    let lowered = raw.to_lowercase();
    let placeholders = ["todo", "tbd", "placeholder", "description", "xxx", "fixme", "n/a"];
    if placeholders.iter().any(|p| lowered == *p || lowered.starts_with(&format!("{p} "))) {
        return Err(format!(
            "description looks like a placeholder ('{raw}'). Write a real description that \
             describes the {kind}'s purpose and contents — it will be shown to every future \
             client that connects."
        ));
    }
    Ok(raw.to_string())
}

/// Strip a leading H1 that duplicates the page name (ported: BookStack shows
/// the page name as the title, so a same-text `# Heading` doubles it).
fn strip_duplicate_title(content: &str, page_name: &str) -> String {
    let trimmed = content.trim_start();
    if let Some(rest) = trimmed.strip_prefix('#') {
        if !rest.starts_with('#') {
            let heading_text = rest.trim();
            let first_line = heading_text.lines().next().unwrap_or("");
            if first_line.trim().eq_ignore_ascii_case(page_name.trim()) {
                let after_heading = heading_text.strip_prefix(first_line).unwrap_or("");
                return after_heading
                    .trim_start_matches('\n')
                    .trim_start_matches('\r')
                    .to_string();
            }
        }
    }
    content.to_string()
}

/// Replace a section in markdown content by heading (ported verbatim).
fn replace_section_markdown(
    md: &str,
    heading: &str,
    content: &str,
    page_id: i64,
) -> Result<String, String> {
    let lines: Vec<&str> = md.lines().collect();
    let heading_pattern = heading.trim_start_matches('#').trim();

    let start = lines
        .iter()
        .position(|line| {
            let trimmed = line.trim_start_matches('#').trim();
            trimmed.eq_ignore_ascii_case(heading_pattern)
        })
        .ok_or(format!("Heading '{heading}' not found in page {page_id}"))?;

    let level = lines[start].chars().take_while(|c| *c == '#').count();

    let end = lines[start + 1..]
        .iter()
        .position(|line| {
            let l = line.chars().take_while(|c| *c == '#').count();
            l > 0 && l <= level
        })
        .map(|p| p + start + 1)
        .unwrap_or(lines.len());

    let mut updated = lines[..=start].join("\n");
    updated.push('\n');
    updated.push_str(content);
    updated.push('\n');
    if end < lines.len() {
        updated.push('\n');
        updated.push_str(&lines[end..].join("\n"));
    }
    Ok(updated)
}

fn format_json(value: &Value) -> ToolResult {
    serde_json::to_string_pretty(value).map_err(|e| e.to_string())
}

fn to_value<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

// --- slim success formatters (ported, using this instance's SPA routes) ---

fn format_page_success(action: &str, details: &pages::PageDetails, base_url: &str) -> String {
    let page = &details.page;
    let url = format!("{base_url}/book/{}/page/{}", details.book_slug, page.slug);
    format!(
        "{action}\nPage ID: {}\nBook ID: {}\nName: {}\nEditor: markdown\nSlug: {}\nRevision: {}\nURL: {url}\nUse get_page({}) to verify content if needed.",
        page.id, page.book_id, page.name, page.slug, page.revision_count, page.id
    )
}

fn format_shelf_success(action: &str, details: &shelves::ShelfDetails, base_url: &str) -> String {
    let shelf = &details.shelf;
    let desc_line = if shelf.description.is_empty() {
        String::new()
    } else {
        format!("\nDescription: {}", shelf.description)
    };
    format!(
        "{action}\nShelf ID: {}\nName: {}\nSlug: {}{desc_line}\nURL: {base_url}/shelf/{}",
        shelf.id, shelf.name, shelf.slug, shelf.slug
    )
}

fn format_book_success(action: &str, details: &books::BookDetails, base_url: &str) -> String {
    let book = &details.book;
    let desc_line = if book.description.is_empty() {
        String::new()
    } else {
        format!("\nDescription: {}", book.description)
    };
    format!(
        "{action}\nBook ID: {}\nName: {}\nSlug: {}{desc_line}\nURL: {base_url}/book/{}",
        book.id, book.name, book.slug, book.slug
    )
}

fn format_chapter_success(action: &str, details: &chapters::ChapterDetails, _base_url: &str) -> String {
    let chapter = &details.chapter;
    let desc_line = if chapter.description.is_empty() {
        String::new()
    } else {
        format!("\nDescription: {}", chapter.description)
    };
    format!(
        "{action}\nChapter ID: {}\nBook ID: {}\nName: {}\nSlug: {}{desc_line}",
        chapter.id, chapter.book_id, chapter.name, chapter.slug
    )
}

// --- schema helpers (ported) ---

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

fn paginated_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "count": { "type": "integer", "description": "Number of results", "default": 50 },
            "offset": { "type": "integer", "description": "Number to skip", "default": 0 }
        }
    })
}

fn id_schema(id_name: &str) -> Value {
    json!({
        "type": "object",
        "properties": {
            id_name: { "type": "integer", "description": format!("The {id_name}") }
        },
        "required": [id_name]
    })
}

fn name_desc_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "description": "Name" },
            "description": {
                "type": "string",
                "description": "REQUIRED. 1-2 sentences on what lives here and what it's for. No placeholders ('TODO', 'description', 'n/a')."
            }
        },
        "required": ["name", "description"]
    })
}

fn update_schema(id_name: &str, fields: &[&str]) -> Value {
    let mut props = json!({ id_name: { "type": "integer", "description": format!("The {id_name}") } });
    for &field in fields {
        props[field] = json!({ "type": "string", "description": format!("New {field}") });
    }
    json!({ "type": "object", "properties": props, "required": [id_name] })
}

fn export_schema(id_name: &str) -> Value {
    json!({
        "type": "object",
        "properties": {
            id_name: { "type": "integer", "description": format!("The {id_name}") },
            "format": { "type": "string", "enum": ["markdown", "plaintext", "html"], "description": "Export format", "default": "markdown" }
        },
        "required": [id_name]
    })
}

// --- tool catalog ---

pub fn definitions() -> Vec<Value> {
    vec![
        tool("search_content",
            "Search across all BookStack content (pages, chapters, books, shelves). Supports operators: {type:page}, [tag_name=value], {in_name:term}, {created_by:me}, exact match with quotes.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "page": { "type": "integer", "description": "Page number", "default": 1 },
                    "count": { "type": "integer", "description": "Results per page", "default": 20 }
                },
                "required": ["query"]
            })),
        tool("directory",
            "Return a scoped, depth-limited tree of BookStack content (shelves → books → chapters → pages). \
             Replaces the assemble-it-yourself pattern of calling list_shelves + list_books + list_chapters + list_pages. \
             \n\nScope: omit (or \"all\") for the full tree, or pass {\"shelf\": ID} / {\"book\": ID} / {\"chapter\": ID} to root the walk. \
             Depth: max levels to descend (0 = roots only, omit for unbounded).",
            json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "description": "Root of the walk. Omit or pass \"all\" for the full tree. Object form: exactly one of {\"shelf\": ID}, {\"book\": ID}, {\"chapter\": ID}.",
                        "oneOf": [
                            { "type": "string", "enum": ["all"] },
                            {
                                "type": "object",
                                "properties": {
                                    "shelf":   { "type": "integer", "description": "Shelf ID to root the walk at" },
                                    "book":    { "type": "integer", "description": "Book ID to root the walk at" },
                                    "chapter": { "type": "integer", "description": "Chapter ID to root the walk at" }
                                },
                                "additionalProperties": false
                            },
                            { "type": "null" }
                        ]
                    },
                    "depth": { "type": "integer", "description": "Max depth to descend (0 = roots only). Omit for unbounded." },
                    "include": { "type": "string", "enum": ["meta", "summary", "full"], "description": "Per-node detail level. `meta` returns id + name + slug + kind.", "default": "meta" }
                }
            })),

        // Shelves
        tool("list_shelves", "List all shelves.", paginated_schema()),
        tool("get_shelf", "Get a shelf by ID, including its books.", id_schema("shelf_id")),
        tool("create_shelf", "Create a new shelf.", name_desc_schema()),
        tool("update_shelf", "Update a shelf's name, description, or set which books it contains via the 'books' array (replaces all existing book assignments on this shelf).", json!({
            "type": "object",
            "properties": {
                "shelf_id": { "type": "integer", "description": "The shelf_id" },
                "name": { "type": "string", "description": "New name" },
                "description": { "type": "string", "description": "New description" },
                "books": { "type": "array", "items": { "type": "integer" }, "description": "Array of book IDs to assign to this shelf (replaces current assignments)" }
            },
            "required": ["shelf_id"]
        })),
        tool("delete_shelf", "Delete a shelf. This does NOT delete the books inside it.", id_schema("shelf_id")),

        // Books
        tool("list_books", "List all books.", paginated_schema()),
        tool("get_book", "Get a book by ID, including its chapters and pages.", id_schema("book_id")),
        tool("create_book", "Create a new book.", name_desc_schema()),
        tool("update_book", "Update a book.", update_schema("book_id", &["name", "description"])),
        tool("delete_book", "Delete a book and all its chapters/pages.", id_schema("book_id")),

        // Chapters
        tool("list_chapters", "List all chapters across all books.", paginated_schema()),
        tool("get_chapter", "Get a chapter by ID, including its pages.", id_schema("chapter_id")),
        tool("create_chapter", "Create a new chapter within a book.", json!({
            "type": "object",
            "properties": {
                "book_id": { "type": "integer", "description": "Book ID to create chapter in" },
                "name": { "type": "string", "description": "Chapter name" },
                "description": {
                    "type": "string",
                    "description": "REQUIRED. 1-2 sentences on what this chapter is for. No placeholders."
                }
            },
            "required": ["book_id", "name", "description"]
        })),
        tool("update_chapter", "Update a chapter's name, description, or move it to a different book by providing book_id.", json!({
            "type": "object",
            "properties": {
                "chapter_id": { "type": "integer", "description": "The chapter_id" },
                "name": { "type": "string", "description": "New name" },
                "description": { "type": "string", "description": "New description" },
                "book_id": { "type": "integer", "description": "Move chapter to a different book by providing the target book ID" }
            },
            "required": ["chapter_id"]
        })),
        tool("delete_chapter", "Delete a chapter. Pages inside become book-level pages.", id_schema("chapter_id")),

        // Pages
        tool("list_pages", "List all pages across all books.", paginated_schema()),
        tool("get_page", "Get a page by ID with full content. Response carries `editor` ('markdown' — this instance is markdown-native), `markdown` source, and rendered `html`.", id_schema("page_id")),
        tool("create_page", "Create a new page. Must provide either book_id or chapter_id. Pass content via `markdown` (this instance is markdown-native).", json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Page name" },
                "book_id": { "type": "integer", "description": "Book ID (if not in a chapter)" },
                "chapter_id": { "type": "integer", "description": "Chapter ID (preferred over book_id)" },
                "markdown": { "type": "string", "description": "Page content in markdown (rendered to HTML server-side)", "default": "" }
            },
            "required": ["name"]
        })),
        tool("update_page", "Replace a page's name and/or content, or move it to a different chapter/book. Full rewrite — for surgical edits prefer edit_page, replace_section, append_to_page, or insert_after.", json!({
            "type": "object",
            "properties": {
                "page_id": { "type": "integer", "description": "The page_id" },
                "name": { "type": "string", "description": "New name" },
                "markdown": { "type": "string", "description": "New markdown content" },
                "chapter_id": { "type": "integer", "description": "Move page to a different chapter by providing the target chapter ID" },
                "book_id": { "type": "integer", "description": "Move page to a different book (at book level, not in any chapter) by providing the target book ID" }
            },
            "required": ["page_id"]
        })),
        tool("edit_page", "Exact-string replace in a page's markdown content. Fails if old_text is not found or is ambiguous (multiple matches without replace_all).", json!({
            "type": "object",
            "properties": {
                "page_id": { "type": "integer", "description": "The page_id" },
                "old_text": { "type": "string", "description": "The exact text to find and replace" },
                "new_text": { "type": "string", "description": "The replacement text" },
                "replace_all": { "type": "boolean", "description": "Replace all occurrences (default false)", "default": false }
            },
            "required": ["page_id", "old_text", "new_text"]
        })),
        tool("append_to_page", "Append markdown content to the end of a page. No need to read the page first.", json!({
            "type": "object",
            "properties": {
                "page_id": { "type": "integer", "description": "The page_id" },
                "markdown": { "type": "string", "description": "Markdown content to append" }
            },
            "required": ["page_id", "markdown"]
        })),
        tool("replace_section", "Replace all content under a heading (up to the next heading of same or higher level). No need to read the page first.", json!({
            "type": "object",
            "properties": {
                "page_id": { "type": "integer", "description": "The page_id" },
                "heading": { "type": "string", "description": "The heading text to find (e.g. '## Related' or just 'Related')" },
                "markdown": { "type": "string", "description": "New content for the section (replaces everything between this heading and the next)" }
            },
            "required": ["page_id", "heading", "markdown"]
        })),
        tool("insert_after", "Insert markdown content after a specific line in a page. Anchor matches exact line content (trimmed). No need to read the page first.", json!({
            "type": "object",
            "properties": {
                "page_id": { "type": "integer", "description": "The page_id" },
                "after": { "type": "string", "description": "The exact line content to insert after (e.g. a heading like '## Notes')" },
                "markdown": { "type": "string", "description": "Markdown content to insert" }
            },
            "required": ["page_id", "after", "markdown"]
        })),
        tool("delete_page", "Delete a page (moves to recycle bin).", id_schema("page_id")),
        tool("list_page_revisions", "List the revision history of a page.", json!({
            "type": "object",
            "properties": {
                "page_id": { "type": "integer", "description": "The page_id" },
                "count": { "type": "integer", "description": "Number of results", "default": 50 },
                "offset": { "type": "integer", "description": "Number to skip", "default": 0 }
            },
            "required": ["page_id"]
        })),

        // Move operations
        tool("move_page", "Move a page to a different chapter or book. Only moves — does not modify content. Provide chapter_id to move into a chapter, or book_id to move to book level (not in any chapter).", json!({
            "type": "object",
            "properties": {
                "page_id": { "type": "integer", "description": "The page to move" },
                "chapter_id": { "type": "integer", "description": "Target chapter ID (moves page into this chapter)" },
                "book_id": { "type": "integer", "description": "Target book ID (moves page to book level, outside any chapter)" }
            },
            "required": ["page_id"]
        })),
        tool("move_chapter", "Move a chapter (with all its pages) to a different book.", json!({
            "type": "object",
            "properties": {
                "chapter_id": { "type": "integer", "description": "The chapter to move" },
                "target_book_id": { "type": "integer", "description": "The book to move the chapter into" }
            },
            "required": ["chapter_id", "target_book_id"]
        })),
        tool("move_book_to_shelf", "Move a book to a different shelf. Optionally remove it from a source shelf. Books can appear on multiple shelves — this adds to the target and optionally removes from the source.", json!({
            "type": "object",
            "properties": {
                "book_id": { "type": "integer", "description": "The book to move" },
                "target_shelf_id": { "type": "integer", "description": "The shelf to add the book to" },
                "remove_from_shelf_id": { "type": "integer", "description": "Optional: shelf to remove the book from (for a true move rather than just adding)" }
            },
            "required": ["book_id", "target_shelf_id"]
        })),

        // Exports
        tool("export_page", "Export a page as markdown, plaintext, or html. Returns the raw exported content.", export_schema("page_id")),
        tool("export_chapter", "Export a chapter as markdown, plaintext, or html. Returns all pages in the chapter.", export_schema("chapter_id")),
        tool("export_book", "Export a book as markdown, plaintext, or html. Returns all chapters and pages.", export_schema("book_id")),

        // Comments
        tool("list_comments", "List comments, optionally filtered by page.", json!({
            "type": "object",
            "properties": {
                "page_id": { "type": "integer", "description": "Filter comments by page ID" }
            }
        })),
        tool("get_comment", "Get a comment by ID.", id_schema("comment_id")),
        tool("create_comment", "Create a comment on a page. Provide content as markdown.", json!({
            "type": "object",
            "properties": {
                "page_id": { "type": "integer", "description": "Page ID to comment on" },
                "markdown": { "type": "string", "description": "Comment content in markdown (rendered to HTML server-side)" },
                "parent_id": { "type": "integer", "description": "Parent comment ID for replies" }
            },
            "required": ["page_id", "markdown"]
        })),
        tool("update_comment", "Update a comment. Provide content as markdown.", json!({
            "type": "object",
            "properties": {
                "comment_id": { "type": "integer", "description": "The comment_id" },
                "markdown": { "type": "string", "description": "New comment content in markdown" }
            },
            "required": ["comment_id", "markdown"]
        })),
        tool("delete_comment", "Delete a comment.", id_schema("comment_id")),

        // Recycle Bin
        tool("list_recycle_bin", "List items in the recycle bin.", paginated_schema()),
        tool("restore_recycle_bin_item", "Restore an item from the recycle bin.", id_schema("deletion_id")),
        tool("destroy_recycle_bin_item", "Permanently delete an item from the recycle bin. Cannot be undone.", id_schema("deletion_id")),

        // Users
        tool("list_users", "List all users.", paginated_schema()),
        tool("get_user", "Get a user by ID.", id_schema("user_id")),

        // Roles
        tool("list_roles", "List all roles.", paginated_schema()),
        tool("get_role", "Get a role by ID, including its permissions.", id_schema("role_id")),

        // System
        tool("get_system_info", "Get BookStack instance information (version, etc.).", json!({
            "type": "object", "properties": {}
        })),
    ]
}

// --- static roles (this rewrite uses coarse roles rather than BookStack's
// full RBAC; ids are stable so get_role stays addressable) ---

fn role_json(id: i64) -> Option<Value> {
    match id {
        1 => Some(json!({
            "id": 1, "display_name": "Admin", "system_name": "admin",
            "description": "Full access, including user management.",
            "permissions": ["content-view", "content-create", "content-update", "content-delete", "users-manage", "settings-manage"],
        })),
        2 => Some(json!({
            "id": 2, "display_name": "Editor", "system_name": "editor",
            "description": "Can view and edit all content.",
            "permissions": ["content-view", "content-create", "content-update", "content-delete"],
        })),
        3 => Some(json!({
            "id": 3, "display_name": "Viewer", "system_name": "viewer",
            "description": "Read-only access to all content.",
            "permissions": ["content-view"],
        })),
        _ => None,
    }
}

// --- dispatch ---

pub async fn call(server: &McpServer, user: &AuthUser, name: &str, args: &Value) -> ToolResult {
    let db = &server.core.db;
    let base_url = server.core.config.public_url.clone();

    match name {
        "search_content" => {
            let query = arg_str(args, "query")?;
            let count = arg_i64(args, "count", 20).clamp(1, 100);
            let page = arg_i64(args, "page", 1).max(1);
            let offset = (page - 1) * count;
            let results = search::search(db, &query, &[], count, offset, Some(user.id))
                .await
                .map_err(map_core)?;
            format_json(&to_value(results))
        }
        "directory" => {
            let scope = parse_directory_scope(args)?;
            let depth = arg_i64_opt(args, "depth").and_then(|d| if d < 0 { None } else { Some(d as u32) });
            let include = arg_str_default(args, "include", "meta");
            if !["meta", "summary", "full"].contains(&include.as_str()) {
                return Err("include must be one of: meta, summary, full".to_string());
            }
            let tree = directory::tree(db, scope, depth).await.map_err(map_core)?;
            format_json(&json!({
                "scope": scope_payload(scope),
                "depth": depth,
                "include": include,
                "tree": to_value(tree),
            }))
        }

        // --- shelves ---
        "list_shelves" => format_json(&to_value(
            shelves::list(db, &list_params(args)).await.map_err(map_core)?,
        )),
        "get_shelf" => format_json(&to_value(
            shelves::get(db, arg_i64_required(args, "shelf_id")?).await.map_err(map_core)?,
        )),
        "create_shelf" => {
            require_edit(user)?;
            let input = shelves::CreateShelf {
                name: arg_str(args, "name")?,
                description: require_description(args, "shelf")?,
                books: arg_i64_array(args, "books"),
                tags: vec![],
            };
            let details = shelves::create(db, user.id, &input).await.map_err(map_core)?;
            Ok(format_shelf_success("Shelf created successfully.", &details, &base_url))
        }
        "update_shelf" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "shelf_id")?;
            let books_arg = args.get("books").and_then(|v| v.as_array()).map(|a| {
                a.iter().filter_map(|v| v.as_i64()).collect::<Vec<_>>()
            });
            let input = shelves::UpdateShelf {
                name: arg_str_opt(args, "name"),
                description: arg_str_opt(args, "description"),
                books: books_arg,
                tags: None,
            };
            let details = shelves::update(db, user.id, id, &input).await.map_err(map_core)?;
            Ok(format_shelf_success("Shelf updated successfully.", &details, &base_url))
        }
        "delete_shelf" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "shelf_id")?;
            shelves::delete(db, user.id, id).await.map_err(map_core)?;
            Ok(format!("Shelf {id} deleted (moved to recycle bin)."))
        }

        // --- books ---
        "list_books" => format_json(&to_value(
            books::list(db, &list_params(args)).await.map_err(map_core)?,
        )),
        "get_book" => format_json(&to_value(
            books::get(db, arg_i64_required(args, "book_id")?).await.map_err(map_core)?,
        )),
        "create_book" => {
            require_edit(user)?;
            let input = books::CreateBook {
                name: arg_str(args, "name")?,
                description: require_description(args, "book")?,
                tags: vec![],
            };
            let details = books::create(db, user.id, &input).await.map_err(map_core)?;
            Ok(format_book_success("Book created successfully.", &details, &base_url))
        }
        "update_book" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "book_id")?;
            let input = books::UpdateBook {
                name: arg_str_opt(args, "name"),
                description: arg_str_opt(args, "description"),
                tags: None,
            };
            let details = books::update(db, user.id, id, &input).await.map_err(map_core)?;
            Ok(format_book_success("Book updated successfully.", &details, &base_url))
        }
        "delete_book" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "book_id")?;
            books::delete(db, user.id, id).await.map_err(map_core)?;
            Ok(format!("Book {id} deleted (moved to recycle bin)."))
        }

        // --- chapters ---
        "list_chapters" => format_json(&to_value(
            chapters::list(db, &list_params(args), arg_i64_opt(args, "book_id"))
                .await
                .map_err(map_core)?,
        )),
        "get_chapter" => format_json(&to_value(
            chapters::get(db, arg_i64_required(args, "chapter_id")?).await.map_err(map_core)?,
        )),
        "create_chapter" => {
            require_edit(user)?;
            let input = chapters::CreateChapter {
                book_id: arg_i64_required(args, "book_id")?,
                name: arg_str(args, "name")?,
                description: require_description(args, "chapter")?,
                tags: vec![],
            };
            let details = chapters::create(db, user.id, &input).await.map_err(map_core)?;
            Ok(format_chapter_success("Chapter created successfully.", &details, &base_url))
        }
        "update_chapter" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "chapter_id")?;
            let input = chapters::UpdateChapter {
                name: arg_str_opt(args, "name"),
                description: arg_str_opt(args, "description"),
                priority: None,
                tags: None,
            };
            let mut details = chapters::update(db, user.id, id, &input).await.map_err(map_core)?;
            if let Some(target_book) = arg_i64_opt(args, "book_id") {
                details = chapters::move_to_book(db, user.id, id, target_book)
                    .await
                    .map_err(map_core)?;
            }
            Ok(format_chapter_success("Chapter updated successfully.", &details, &base_url))
        }
        "delete_chapter" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "chapter_id")?;
            chapters::delete(db, user.id, id).await.map_err(map_core)?;
            Ok(format!("Chapter {id} deleted (moved to recycle bin). Its pages became book-level pages."))
        }

        // --- pages ---
        "list_pages" => format_json(&to_value(
            pages::list(
                db,
                &list_params(args),
                arg_i64_opt(args, "book_id"),
                arg_i64_opt(args, "chapter_id"),
            )
            .await
            .map_err(map_core)?,
        )),
        "get_page" => {
            let details = pages::get(db, arg_i64_required(args, "page_id")?)
                .await
                .map_err(map_core)?;
            let url = format!("{base_url}/book/{}/page/{}", details.book_slug, details.page.slug);
            let mut value = to_value(details);
            if let Some(obj) = value.as_object_mut() {
                obj.insert("editor".to_string(), json!("markdown"));
                obj.insert("url".to_string(), json!(url));
            }
            format_json(&value)
        }
        "create_page" => {
            require_edit(user)?;
            if args.get("html").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty()) {
                return Err("this instance is markdown-native — pass content via the 'markdown' parameter".to_string());
            }
            let name = arg_str(args, "name")?;
            let chapter_id = arg_i64_opt(args, "chapter_id");
            let book_id = match (arg_i64_opt(args, "book_id"), chapter_id) {
                (_, Some(cid)) => chapters::fetch(db, cid).await.map_err(map_core)?.book_id,
                (Some(bid), None) => bid,
                (None, None) => return Err("Either book_id or chapter_id is required".to_string()),
            };
            let markdown = strip_duplicate_title(&arg_str_default(args, "markdown", ""), &name);
            let input = pages::CreatePage {
                book_id,
                chapter_id,
                name,
                markdown,
                draft: false,
                tags: vec![],
            };
            let details = pages::create(db, user.id, &input).await.map_err(map_core)?;
            Ok(format_page_success("Page created successfully.", &details, &base_url))
        }
        "update_page" => {
            require_edit(user)?;
            if args.get("html").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty()) {
                return Err("this instance is markdown-native — pass content via the 'markdown' parameter".to_string());
            }
            let id = arg_i64_required(args, "page_id")?;
            let move_chapter = arg_i64_opt(args, "chapter_id");
            let move_book = arg_i64_opt(args, "book_id");
            if move_chapter.is_some() && move_book.is_some() {
                return Err("Provide either chapter_id or book_id, not both".to_string());
            }

            let name_arg = arg_str_opt(args, "name");
            let markdown_arg = arg_str_opt(args, "markdown").filter(|s| !s.is_empty());
            let mut details = if name_arg.is_some() || markdown_arg.is_some() {
                let page_name = match &name_arg {
                    Some(n) => n.clone(),
                    None => pages::fetch(db, id).await.map_err(map_core)?.name,
                };
                let input = pages::UpdatePage {
                    name: name_arg,
                    markdown: markdown_arg.map(|md| strip_duplicate_title(&md, &page_name)),
                    summary: Some("Updated via MCP".to_string()),
                    ..Default::default()
                };
                let (details, content_changed) =
                    pages::update(db, user.id, id, &input).await.map_err(map_core)?;
                if content_changed {
                    server.collab.invalidate(id).await;
                }
                details
            } else {
                pages::get(db, id).await.map_err(map_core)?
            };

            if move_chapter.is_some() || move_book.is_some() {
                let (target_book, target_chapter) = match (move_chapter, move_book) {
                    (Some(cid), _) => (chapters::fetch(db, cid).await.map_err(map_core)?.book_id, Some(cid)),
                    (None, Some(bid)) => (bid, None),
                    (None, None) => unreachable!(),
                };
                details = pages::move_page(db, user.id, id, target_book, target_chapter)
                    .await
                    .map_err(map_core)?;
            }
            Ok(format_page_success("Page updated successfully.", &details, &base_url))
        }
        "edit_page" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "page_id")?;
            let old_text = arg_str(args, "old_text")?;
            let new_text = arg_str(args, "new_text")?;
            let replace_all = arg_bool(args, "replace_all", false);

            let page = pages::fetch(db, id).await.map_err(map_core)?;
            let count = page.markdown.matches(&old_text).count();
            if count == 0 {
                return Err(format!(
                    "old_text not found in page {id}. Make sure old_text matches the 'markdown' field from get_page."
                ));
            }
            if count > 1 && !replace_all {
                return Err(format!(
                    "old_text found {count} times in page {id}. Use replace_all=true to replace all, or provide more context to make it unique."
                ));
            }
            let updated = if replace_all {
                page.markdown.replace(&old_text, &new_text)
            } else {
                page.markdown.replacen(&old_text, &new_text, 1)
            };
            let input = pages::UpdatePage {
                markdown: Some(updated),
                summary: Some("Edited via MCP (edit_page)".to_string()),
                ..Default::default()
            };
            let (details, content_changed) =
                pages::update(db, user.id, id, &input).await.map_err(map_core)?;
            if content_changed {
                server.collab.invalidate(id).await;
            }
            Ok(format_page_success("Page updated successfully.", &details, &base_url))
        }
        "append_to_page" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "page_id")?;
            let content = arg_str(args, "markdown")?;
            let page = pages::fetch(db, id).await.map_err(map_core)?;
            let updated = format!("{}\n\n{}", page.markdown.trim_end(), content);
            let input = pages::UpdatePage {
                markdown: Some(updated),
                summary: Some("Appended via MCP".to_string()),
                ..Default::default()
            };
            let (details, content_changed) =
                pages::update(db, user.id, id, &input).await.map_err(map_core)?;
            if content_changed {
                server.collab.invalidate(id).await;
            }
            Ok(format_page_success("Content appended successfully.", &details, &base_url))
        }
        "replace_section" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "page_id")?;
            let heading = arg_str(args, "heading")?;
            let content = arg_str(args, "markdown")?;
            let page = pages::fetch(db, id).await.map_err(map_core)?;
            let updated = replace_section_markdown(&page.markdown, &heading, &content, id)?;
            let input = pages::UpdatePage {
                markdown: Some(updated),
                summary: Some(format!("Replaced section '{heading}' via MCP")),
                ..Default::default()
            };
            let (details, content_changed) =
                pages::update(db, user.id, id, &input).await.map_err(map_core)?;
            if content_changed {
                server.collab.invalidate(id).await;
            }
            Ok(format_page_success("Section replaced successfully.", &details, &base_url))
        }
        "insert_after" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "page_id")?;
            let after = arg_str(args, "after")?;
            let content = arg_str(args, "markdown")?;
            let page = pages::fetch(db, id).await.map_err(map_core)?;

            let lines: Vec<&str> = page.markdown.lines().collect();
            let pos = lines
                .iter()
                .position(|line| line.trim() == after.trim())
                .ok_or(format!(
                    "Anchor '{after}' not found in page {id}. Make sure the anchor matches a line from the 'markdown' field."
                ))?;
            let mut updated = lines[..=pos].join("\n");
            updated.push('\n');
            updated.push_str(&content);
            updated.push('\n');
            if pos + 1 < lines.len() {
                updated.push_str(&lines[pos + 1..].join("\n"));
            }

            let input = pages::UpdatePage {
                markdown: Some(updated),
                summary: Some("Inserted content via MCP".to_string()),
                ..Default::default()
            };
            let (details, content_changed) =
                pages::update(db, user.id, id, &input).await.map_err(map_core)?;
            if content_changed {
                server.collab.invalidate(id).await;
            }
            Ok(format_page_success("Content inserted successfully.", &details, &base_url))
        }
        "delete_page" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "page_id")?;
            pages::delete(db, user.id, id).await.map_err(map_core)?;
            server.collab.invalidate(id).await;
            Ok(format!("Page {id} deleted (moved to recycle bin)."))
        }
        "list_page_revisions" => format_json(&to_value(
            pages::revisions(db, arg_i64_required(args, "page_id")?, &list_params(args))
                .await
                .map_err(map_core)?,
        )),

        // --- moves ---
        "move_page" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "page_id")?;
            let chapter_id = arg_i64_opt(args, "chapter_id");
            let book_id = arg_i64_opt(args, "book_id");
            let (target_book, target_chapter) = match (chapter_id, book_id) {
                (Some(_), Some(_)) => return Err("Provide either chapter_id or book_id, not both".to_string()),
                (Some(cid), None) => (chapters::fetch(db, cid).await.map_err(map_core)?.book_id, Some(cid)),
                (None, Some(bid)) => (bid, None),
                (None, None) => return Err("Either chapter_id or book_id is required".to_string()),
            };
            let details = pages::move_page(db, user.id, id, target_book, target_chapter)
                .await
                .map_err(map_core)?;
            server.collab.invalidate(id).await;
            Ok(format_page_success("Page moved successfully.", &details, &base_url))
        }
        "move_chapter" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "chapter_id")?;
            let target = arg_i64_required(args, "target_book_id")?;
            let details = chapters::move_to_book(db, user.id, id, target).await.map_err(map_core)?;
            Ok(format_chapter_success("Chapter moved successfully.", &details, &base_url))
        }
        "move_book_to_shelf" => {
            require_edit(user)?;
            let book_id = arg_i64_required(args, "book_id")?;
            let target = arg_i64_required(args, "target_shelf_id")?;
            shelves::add_book(db, target, book_id).await.map_err(map_core)?;
            let removed = if let Some(source) = arg_i64_opt(args, "remove_from_shelf_id") {
                shelves::remove_book(db, source, book_id).await.map_err(map_core)?;
                format!(" and removed from shelf {source}")
            } else {
                String::new()
            };
            Ok(format!("Book {book_id} added to shelf {target}{removed}."))
        }

        // --- exports ---
        "export_page" => {
            let format = exports::ExportFormat::parse(&arg_str_default(args, "format", "markdown"))
                .map_err(map_core)?;
            exports::export_page(db, arg_i64_required(args, "page_id")?, format)
                .await
                .map_err(map_core)
        }
        "export_chapter" => {
            let format = exports::ExportFormat::parse(&arg_str_default(args, "format", "markdown"))
                .map_err(map_core)?;
            exports::export_chapter(db, arg_i64_required(args, "chapter_id")?, format)
                .await
                .map_err(map_core)
        }
        "export_book" => {
            let format = exports::ExportFormat::parse(&arg_str_default(args, "format", "markdown"))
                .map_err(map_core)?;
            exports::export_book(db, arg_i64_required(args, "book_id")?, format)
                .await
                .map_err(map_core)
        }

        // --- comments ---
        "list_comments" => format_json(&to_value(
            comments::list(db, arg_i64_opt(args, "page_id")).await.map_err(map_core)?,
        )),
        "get_comment" => format_json(&to_value(
            comments::get(db, arg_i64_required(args, "comment_id")?).await.map_err(map_core)?,
        )),
        "create_comment" => {
            require_edit(user)?;
            let comment = comments::create(
                db,
                user.id,
                arg_i64_required(args, "page_id")?,
                &arg_str(args, "markdown")?,
                arg_i64_opt(args, "parent_id"),
            )
            .await
            .map_err(map_core)?;
            format_json(&to_value(comment))
        }
        "update_comment" => {
            require_edit(user)?;
            let comment = comments::update(
                db,
                user.id,
                arg_i64_required(args, "comment_id")?,
                &arg_str(args, "markdown")?,
            )
            .await
            .map_err(map_core)?;
            format_json(&to_value(comment))
        }
        "delete_comment" => {
            require_edit(user)?;
            let id = arg_i64_required(args, "comment_id")?;
            comments::delete(db, id).await.map_err(map_core)?;
            Ok(format!("Comment {id} deleted."))
        }

        // --- recycle bin ---
        "list_recycle_bin" => format_json(&to_value(
            recycle::list(db, &list_params(args)).await.map_err(map_core)?,
        )),
        "restore_recycle_bin_item" => {
            require_edit(user)?;
            let deletion = recycle::restore(db, arg_i64_required(args, "deletion_id")?)
                .await
                .map_err(map_core)?;
            Ok(format!(
                "Restored {} '{}' (ID {}).",
                deletion.entity_type, deletion.entity_name, deletion.entity_id
            ))
        }
        "destroy_recycle_bin_item" => {
            require_edit(user)?;
            let deletion = recycle::destroy(db, arg_i64_required(args, "deletion_id")?)
                .await
                .map_err(map_core)?;
            Ok(format!(
                "Permanently deleted {} '{}' (ID {}). This cannot be undone.",
                deletion.entity_type, deletion.entity_name, deletion.entity_id
            ))
        }

        // --- users / roles / system ---
        "list_users" => format_json(&to_value(
            users::list(db, &list_params(args)).await.map_err(map_core)?,
        )),
        "get_user" => format_json(&to_value(
            users::get(db, arg_i64_required(args, "user_id")?).await.map_err(map_core)?,
        )),
        "list_roles" => format_json(&json!({
            "data": [role_json(1), role_json(2), role_json(3)],
            "total": 3,
        })),
        "get_role" => {
            let id = arg_i64_required(args, "role_id")?;
            match role_json(id) {
                Some(role) => format_json(&role),
                None => Err("not found".to_string()),
            }
        }
        "get_system_info" => format_json(&to_value(system::info(db).await.map_err(map_core)?)),

        _ => Err(format!("unknown tool: {name}")),
    }
}

fn parse_directory_scope(args: &Value) -> Result<directory::Scope, String> {
    match args.get("scope") {
        None | Some(Value::Null) => Ok(directory::Scope::All),
        Some(Value::String(s)) if s == "all" => Ok(directory::Scope::All),
        Some(Value::Object(map)) => {
            let keys: Vec<&String> = map.keys().collect();
            if keys.len() != 1 {
                return Err("scope object must contain exactly one of: shelf, book, chapter".to_string());
            }
            let id = map.values().next().and_then(|v| v.as_i64()).ok_or("scope ID must be an integer")?;
            match keys[0].as_str() {
                "shelf" => Ok(directory::Scope::Shelf(id)),
                "book" => Ok(directory::Scope::Book(id)),
                "chapter" => Ok(directory::Scope::Chapter(id)),
                other => Err(format!("unknown scope key: {other}")),
            }
        }
        Some(_) => Err("scope must be \"all\" or an object like {\"book\": 1}".to_string()),
    }
}

fn scope_payload(scope: directory::Scope) -> Value {
    match scope {
        directory::Scope::All => json!("all"),
        directory::Scope::Shelf(id) => json!({ "shelf": id }),
        directory::Scope::Book(id) => json!({ "book": id }),
        directory::Scope::Chapter(id) => json!({ "chapter": id }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_list_count_is_46() {
        assert_eq!(definitions().len(), 46);
    }

    #[test]
    fn replace_section_bounds() {
        let md = "intro\n\n## One\na\nb\n\n## Two\nc\n\n# Top\nd";
        let out = replace_section_markdown(md, "One", "NEW", 1).unwrap();
        assert!(out.contains("## One\nNEW\n\n## Two"));
        // Replacing "Two" (h2) consumes up to "# Top" (h1 ends it).
        let out2 = replace_section_markdown(md, "## Two", "X", 1).unwrap();
        assert!(out2.contains("## Two\nX\n\n# Top"));
        assert!(replace_section_markdown(md, "Missing", "x", 1).is_err());
    }

    #[test]
    fn strip_duplicate_title_works() {
        assert_eq!(strip_duplicate_title("# My Page\n\nbody", "My Page"), "body");
        assert_eq!(strip_duplicate_title("# Other\n\nbody", "My Page"), "# Other\n\nbody");
        assert_eq!(strip_duplicate_title("## My Page\nbody", "My Page"), "## My Page\nbody");
    }

    #[test]
    fn description_validation() {
        assert!(require_description(&json!({}), "book").is_err());
        assert!(require_description(&json!({"description": "todo"}), "book").is_err());
        assert!(require_description(&json!({"description": "short"}), "book").is_err());
        assert!(require_description(
            &json!({"description": "Engineering runbooks and operational procedures."}),
            "book"
        )
        .is_ok());
    }
}
