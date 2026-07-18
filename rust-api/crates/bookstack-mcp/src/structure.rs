//! Initialize instructions: usage guidance plus a live structure tree of the
//! knowledge base, ported from bees-roadhouse/bookstack-mcp so connecting
//! clients get the same routing context.

use sqlx::PgPool;

use bookstack_core::Core;

pub async fn instructions(core: &Core, org_id: i64) -> String {
    let mut out = String::new();

    out.push_str(
        "BookStack knowledge management server. Content is organized as: \
         Shelves > Books > Chapters > Pages. Use search_content to find content by \
         keyword or tag, semantic navigation via the directory tool, or navigate the \
         hierarchy using the IDs in the structure below.\n\n",
    );

    out.push_str(
        "IMPORTANT: Before creating or updating any page, first retrieve an existing page \
         from the same book or chapter using get_page to identify the writing style, \
         formatting conventions, heading structure, and markdown patterns already in use. \
         Match the established style of the surrounding content.\n\n\
         IMPORTANT: Validate content placement before creating pages. Each shelf, book, and \
         chapter has a specific purpose described in the structure below. Do NOT place content \
         where it doesn't belong. If the user asks to create content in a location that doesn't \
         match the target's purpose, push back and suggest the correct location. When unsure, \
         check the shelf/book/chapter descriptions using get_shelf, get_book, or get_chapter.\n\n\
         IMPORTANT: Descriptions on shelves, books, and chapters are REQUIRED, not optional. \
         When you call create_shelf, create_book, or create_chapter, you MUST provide a \
         meaningful 1-2 sentence description. Descriptions are surfaced to every client \
         that connects to this BookStack — they literally shape how future content gets \
         routed. Do NOT use placeholders like 'TODO', 'description', or 'n/a' — the server \
         will reject them.\n\n\
         This instance is markdown-native: every page stores markdown source and the server \
         renders sanitized HTML automatically. Send page and comment content via the \
         'markdown' parameter.\n\n\
         IMPORTANT: The page name is displayed as an H1 title at the top of every page. Do \
         NOT include the page title as a heading (e.g. '# Page Name') in the markdown content \
         — this causes a duplicate title. Start content directly with body text or a \
         sub-heading (## or lower).\n\n\
         All editing tools (edit_page, replace_section, append_to_page, insert_after) operate \
         on the page's markdown source. Prefer these targeted tools over update_page for \
         partial edits — update_page rewrites the entire page and should only be used when \
         the whole page needs replacing.\n\n\
         Pages may be open in realtime collaborative editing sessions. Content changes made \
         through these tools take effect immediately; live editors are transparently \
         reconnected to the new content.\n\n",
    );

    let public_url = &core.config.public_url;
    out.push_str(&format!(
        "BookStack URL: {public_url}\n\
         When you create or update content, present a clickable link to the user so they can \
         review it. URL patterns:\n\
         - Pages: {public_url}/book/{{book_slug}}/page/{{page_slug}}\n\
         - Books: {public_url}/book/{{slug}}\n\
         - Shelves: {public_url}/shelf/{{slug}}\n\n"
    ));

    match build_structure(&core.db, org_id).await {
        Some(structure) => {
            out.push_str("Current structure:\n\n");
            out.push_str(&structure);
        }
        None => out.push_str("Use list_shelves and list_books to explore the structure."),
    }

    out
}

/// Render the shelf → book → chapter tree with IDs and truncated
/// descriptions. Books on no shelf are listed under "(unshelved)".
async fn build_structure(db: &PgPool, org_id: i64) -> Option<String> {
    let shelves: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, name, description FROM shelves WHERE org_id = $1 AND deleted_at IS NULL ORDER BY name")
            .bind(org_id)
            .fetch_all(db)
            .await
            .ok()?;
    let shelf_books: Vec<(i64, i64)> =
        sqlx::query_as(r#"SELECT sb.shelf_id, sb.book_id FROM shelf_books sb JOIN shelves s ON s.id = sb.shelf_id WHERE s.org_id = $1 ORDER BY sb.shelf_id, sb."order""#)
            .bind(org_id)
            .fetch_all(db)
            .await
            .ok()?;
    let books: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, name, description FROM books WHERE org_id = $1 AND deleted_at IS NULL ORDER BY name")
            .bind(org_id)
            .fetch_all(db)
            .await
            .ok()?;
    let chapters: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT id, book_id, name, description FROM chapters WHERE org_id = $1 AND deleted_at IS NULL ORDER BY priority, id",
    )
    .bind(org_id)
    .fetch_all(db)
    .await
    .ok()?;

    if shelves.is_empty() && books.is_empty() {
        return None;
    }

    let mut out = String::new();
    let render_book = |out: &mut String, id: i64, name: &str, desc: &str, indent: &str| {
        let desc = truncate_desc(desc);
        if desc.is_empty() {
            out.push_str(&format!("{indent}Book: {name} (ID: {id})\n"));
        } else {
            out.push_str(&format!("{indent}Book: {name} (ID: {id}) — {desc}\n"));
        }
        for (cid, book_id, cname, cdesc) in &chapters {
            if *book_id == id {
                let cdesc = truncate_desc(cdesc);
                if cdesc.is_empty() {
                    out.push_str(&format!("{indent}  Chapter: {cname} (ID: {cid})\n"));
                } else {
                    out.push_str(&format!("{indent}  Chapter: {cname} (ID: {cid}) — {cdesc}\n"));
                }
            }
        }
    };

    let mut shelved: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for (shelf_id, name, description) in &shelves {
        let desc = truncate_desc(description);
        if desc.is_empty() {
            out.push_str(&format!("Shelf: {name} (ID: {shelf_id})\n"));
        } else {
            out.push_str(&format!("Shelf: {name} (ID: {shelf_id}) — {desc}\n"));
        }
        for (sid, book_id) in &shelf_books {
            if sid == shelf_id {
                shelved.insert(*book_id);
                if let Some((id, bname, bdesc)) = books.iter().find(|(bid, _, _)| bid == book_id) {
                    render_book(&mut out, *id, bname, bdesc, "  ");
                }
            }
        }
        out.push('\n');
    }

    let unshelved: Vec<_> = books.iter().filter(|(id, _, _)| !shelved.contains(id)).collect();
    if !unshelved.is_empty() {
        out.push_str("(unshelved)\n");
        for (id, name, desc) in unshelved {
            render_book(&mut out, *id, name, desc, "  ");
        }
    }

    Some(out)
}

/// Truncate a description for the structure tree: collapse whitespace, cap at
/// 150 chars.
fn truncate_desc(desc: &str) -> String {
    let collapsed: String = desc.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= 150 {
        collapsed
    } else {
        let truncated: String = collapsed.chars().take(147).collect();
        format!("{truncated}...")
    }
}
