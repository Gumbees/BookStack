use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Editor,
    Viewer,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Editor => "editor",
            Role::Viewer => "viewer",
        }
    }

    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "admin" => Some(Role::Admin),
            "editor" => Some(Role::Editor),
            "viewer" => Some(Role::Viewer),
            _ => None,
        }
    }

    pub fn can_edit(&self) -> bool {
        matches!(self, Role::Admin | Role::Editor)
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, Role::Admin)
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    #[serde(skip)]
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn role_enum(&self) -> Role {
        Role::parse(&self.role).unwrap_or(Role::Viewer)
    }
}

/// Authenticated principal attached to a request (via JWT or API token).
#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub role: Role,
}

impl From<&User> for AuthUser {
    fn from(u: &User) -> Self {
        AuthUser {
            id: u.id,
            name: u.name.clone(),
            email: u.email.clone(),
            role: u.role_enum(),
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ApiToken {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub token_id: String,
    #[serde(skip)]
    pub secret_hash: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Shelf {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub created_by: Option<i64>,
    pub updated_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Book {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub created_by: Option<i64>,
    pub updated_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Chapter {
    pub id: i64,
    pub book_id: i64,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub priority: i32,
    pub created_by: Option<i64>,
    pub updated_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Page metadata without content bodies (used in listings and trees).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PageMeta {
    pub id: i64,
    pub book_id: i64,
    pub chapter_id: Option<i64>,
    pub name: String,
    pub slug: String,
    pub priority: i32,
    pub draft: bool,
    pub revision_count: i32,
    pub created_by: Option<i64>,
    pub updated_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Page {
    pub id: i64,
    pub book_id: i64,
    pub chapter_id: Option<i64>,
    pub name: String,
    pub slug: String,
    pub markdown: String,
    pub html: String,
    pub priority: i32,
    pub draft: bool,
    pub revision_count: i32,
    pub created_by: Option<i64>,
    pub updated_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Explicit column list for `Page` selects (skips `ydoc_state` and
/// `search_vector`, which are large and not part of the API surface).
pub const PAGE_COLS: &str = "id, book_id, chapter_id, name, slug, markdown, html, priority, draft, revision_count, created_by, updated_by, created_at, updated_at";

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PageRevision {
    pub id: i64,
    pub page_id: i64,
    pub revision_number: i32,
    pub name: String,
    pub markdown: String,
    pub summary: String,
    pub created_by: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Tag {
    #[serde(skip_deserializing)]
    pub id: i64,
    #[serde(skip_deserializing)]
    pub entity_type: String,
    #[serde(skip_deserializing)]
    pub entity_id: i64,
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default, rename = "order")]
    #[sqlx(rename = "order")]
    pub order: i32,
}

/// One entry in a book's contents tree.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentItem {
    Chapter {
        #[serde(flatten)]
        chapter: Chapter,
        pages: Vec<PageMeta>,
    },
    Page {
        #[serde(flatten)]
        page: PageMeta,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub entity_type: String,
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub book_slug: Option<String>,
    pub preview: String,
    pub rank: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Paginated<T> {
    pub data: Vec<T>,
    pub total: i64,
}

/// Common listing controls: `?count=&offset=&sort=&order=`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListParams {
    pub count: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

impl ListParams {
    pub fn limit(&self) -> i64 {
        self.count.unwrap_or(100).clamp(1, 500)
    }

    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }

    /// Resolve the sort column against a whitelist; defaults to the first entry.
    pub fn sort_sql(&self, allowed: &[&'static str]) -> String {
        let col = self
            .sort
            .as_deref()
            .and_then(|s| allowed.iter().find(|a| **a == s))
            .copied()
            .unwrap_or(allowed[0]);
        let dir = match self.order.as_deref() {
            Some("desc") => "DESC",
            _ => "ASC",
        };
        format!("{col} {dir}")
    }
}
