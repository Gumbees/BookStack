//! Import content from a live BookStack instance over its REST API,
//! respecting API rate limits: a client-side request pacer (default 90
//! req/min, configurable ≤ the instance's limit — BookStack ships 180/min)
//! plus honoring 429 `Retry-After` with bounded retries.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{json, Value};
use sqlx::PgPool;

use bookstack_core::models::Tag;
use bookstack_core::services::{books, chapters, pages, shelves};

use crate::{OpsError, Result};

pub struct ImportConfig {
    pub base_url: String,
    pub token_id: String,
    pub token_secret: String,
    pub org_id: i64,
    pub user_id: i64,
    pub rate_limit_per_minute: u32,
}

struct SourceClient {
    http: reqwest::Client,
    base: String,
    auth: String,
    min_interval: Duration,
    last_request: tokio::sync::Mutex<Option<tokio::time::Instant>>,
}

impl SourceClient {
    fn new(config: &ImportConfig) -> SourceClient {
        let rpm = config.rate_limit_per_minute.clamp(6, 600);
        SourceClient {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
            base: config.base_url.trim_end_matches('/').to_string(),
            auth: format!("Token {}:{}", config.token_id, config.token_secret),
            min_interval: Duration::from_secs_f64(60.0 / rpm as f64),
            last_request: tokio::sync::Mutex::new(None),
        }
    }

    /// Pace requests to the configured rate, then GET with 429-aware retries.
    async fn get_raw(&self, path: &str) -> Result<reqwest::Response> {
        for attempt in 0..6 {
            {
                let mut last = self.last_request.lock().await;
                if let Some(previous) = *last {
                    let elapsed = previous.elapsed();
                    if elapsed < self.min_interval {
                        tokio::time::sleep(self.min_interval - elapsed).await;
                    }
                }
                *last = Some(tokio::time::Instant::now());
            }
            let response = self
                .http
                .get(format!("{}{path}", self.base))
                .header("Authorization", &self.auth)
                .send()
                .await
                .map_err(|e| OpsError::Import(format!("request failed: {e}")))?;

            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let wait = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(15)
                    .min(120);
                tracing::info!("source rate-limited (429); waiting {wait}s (attempt {attempt})");
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }
            if !response.status().is_success() {
                return Err(OpsError::Import(format!(
                    "source returned {} for {path}",
                    response.status()
                )));
            }
            return Ok(response);
        }
        Err(OpsError::Import("giving up after repeated 429 rate-limit responses".into()))
    }

    async fn get_json(&self, path: &str) -> Result<Value> {
        self.get_raw(path)
            .await?
            .json()
            .await
            .map_err(|e| OpsError::Import(format!("invalid JSON from {path}: {e}")))
    }

    async fn get_text(&self, path: &str) -> Result<String> {
        self.get_raw(path)
            .await?
            .text()
            .await
            .map_err(|e| OpsError::Import(format!("invalid body from {path}: {e}")))
    }

    /// Fetch every record of a paginated listing endpoint.
    async fn list_all(&self, endpoint: &str) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        let mut offset = 0;
        loop {
            let page = self.get_json(&format!("/api/{endpoint}?count=100&offset={offset}")).await?;
            let data = page.get("data").and_then(Value::as_array).cloned().unwrap_or_default();
            let total = page.get("total").and_then(Value::as_i64).unwrap_or(0);
            let fetched = data.len();
            out.extend(data);
            offset += fetched as i64;
            if fetched == 0 || offset >= total {
                break;
            }
        }
        Ok(out)
    }
}

fn tags_of(detail: &Value) -> Vec<Tag> {
    detail
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(|t| {
                    let name = t.get("name").and_then(Value::as_str)?.to_string();
                    Some(Tag {
                        id: 0,
                        entity_type: String::new(),
                        entity_id: 0,
                        name,
                        value: t.get("value").and_then(Value::as_str).unwrap_or("").to_string(),
                        order: 0,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// BookStack markdown exports prepend the page name as an H1; drop it.
fn strip_export_title(markdown: &str, name: &str) -> String {
    let trimmed = markdown.trim_start();
    if let Some(rest) = trimmed.strip_prefix("# ") {
        if let Some(first_line) = rest.lines().next() {
            if first_line.trim().eq_ignore_ascii_case(name.trim()) {
                let after = rest.strip_prefix(first_line).unwrap_or("");
                return after.trim_start_matches(['\n', '\r']).to_string();
            }
        }
    }
    markdown.to_string()
}

async fn update_progress(db: &PgPool, job_id: i64, progress: &Value, status: Option<&str>) {
    let _ = match status {
        Some(s) => {
            sqlx::query("UPDATE import_jobs SET progress = $2, status = $3, updated_at = now() WHERE id = $1")
                .bind(job_id)
                .bind(progress)
                .bind(s)
                .execute(db)
                .await
        }
        None => {
            sqlx::query("UPDATE import_jobs SET progress = $2, updated_at = now() WHERE id = $1")
                .bind(job_id)
                .bind(progress)
                .execute(db)
                .await
        }
    };
}

/// Run a full import; progress lands in import_jobs.progress as it goes.
pub async fn run(db: PgPool, job_id: i64, config: ImportConfig) {
    match run_inner(&db, job_id, &config).await {
        Ok(progress) => update_progress(&db, job_id, &progress, Some("completed")).await,
        Err(err) => {
            let progress = json!({ "fatal_error": err.to_string() });
            update_progress(&db, job_id, &progress, Some("failed")).await;
        }
    }
}

async fn run_inner(db: &PgPool, job_id: i64, config: &ImportConfig) -> Result<Value> {
    let client = SourceClient::new(config);
    let org = config.org_id;
    let user = config.user_id;
    let mut errors: Vec<String> = Vec::new();
    let mut counters = json!({
        "phase": "listing", "books": 0, "chapters": 0, "pages": 0, "shelves": 0, "skipped_drafts": 0,
    });
    update_progress(db, job_id, &counters, None).await;

    let source_books = client.list_all("books").await?;
    let source_chapters = client.list_all("chapters").await?;
    let source_pages = client.list_all("pages").await?;
    let source_shelves = client.list_all("shelves").await?;
    counters["totals"] = json!({
        "books": source_books.len(), "chapters": source_chapters.len(),
        "pages": source_pages.len(), "shelves": source_shelves.len(),
    });

    // --- books ---
    counters["phase"] = json!("books");
    let mut book_map: HashMap<i64, i64> = HashMap::new();
    for source in &source_books {
        let Some(source_id) = source.get("id").and_then(Value::as_i64) else { continue };
        let detail = client.get_json(&format!("/api/books/{source_id}")).await?;
        let input = books::CreateBook {
            name: detail.get("name").and_then(Value::as_str).unwrap_or("Untitled").to_string(),
            description: detail.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
            tags: tags_of(&detail),
        };
        match books::create(db, org, user, &input).await {
            Ok(created) => {
                book_map.insert(source_id, created.book.id);
                counters["books"] = json!(book_map.len());
            }
            Err(e) => errors.push(format!("book {source_id}: {e}")),
        }
        update_progress(db, job_id, &counters, None).await;
    }

    // --- chapters ---
    counters["phase"] = json!("chapters");
    let mut chapter_map: HashMap<i64, i64> = HashMap::new();
    for source in &source_chapters {
        let (Some(source_id), Some(source_book)) = (
            source.get("id").and_then(Value::as_i64),
            source.get("book_id").and_then(Value::as_i64),
        ) else { continue };
        let Some(&target_book) = book_map.get(&source_book) else {
            errors.push(format!("chapter {source_id}: parent book {source_book} was not imported"));
            continue;
        };
        let input = chapters::CreateChapter {
            book_id: target_book,
            name: source.get("name").and_then(Value::as_str).unwrap_or("Untitled").to_string(),
            description: source.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
            tags: vec![],
        };
        match chapters::create(db, org, user, &input).await {
            Ok(created) => {
                chapter_map.insert(source_id, created.chapter.id);
                counters["chapters"] = json!(chapter_map.len());
            }
            Err(e) => errors.push(format!("chapter {source_id}: {e}")),
        }
        update_progress(db, job_id, &counters, None).await;
    }

    // --- pages (detail + markdown export per page) ---
    counters["phase"] = json!("pages");
    let mut imported_pages = 0usize;
    let mut skipped_drafts = 0usize;
    for source in &source_pages {
        let Some(source_id) = source.get("id").and_then(Value::as_i64) else { continue };
        if source.get("draft").and_then(Value::as_bool).unwrap_or(false) {
            skipped_drafts += 1;
            counters["skipped_drafts"] = json!(skipped_drafts);
            continue;
        }
        let detail = match client.get_json(&format!("/api/pages/{source_id}")).await {
            Ok(d) => d,
            Err(e) => {
                errors.push(format!("page {source_id}: {e}"));
                continue;
            }
        };
        let name = detail.get("name").and_then(Value::as_str).unwrap_or("Untitled").to_string();
        // The markdown export converts WYSIWYG/html pages too.
        let markdown = match client.get_text(&format!("/api/pages/{source_id}/export/markdown")).await {
            Ok(text) => strip_export_title(&text, &name),
            Err(_) => detail.get("markdown").and_then(Value::as_str).unwrap_or("").to_string(),
        };
        let source_book = detail.get("book_id").and_then(Value::as_i64).unwrap_or(0);
        let source_chapter = detail.get("chapter_id").and_then(Value::as_i64).filter(|c| *c > 0);
        let Some(&target_book) = book_map.get(&source_book) else {
            errors.push(format!("page {source_id}: parent book {source_book} was not imported"));
            continue;
        };
        let input = pages::CreatePage {
            book_id: target_book,
            chapter_id: source_chapter.and_then(|c| chapter_map.get(&c).copied()),
            name,
            markdown,
            draft: false,
            tags: tags_of(&detail),
        };
        match pages::create(db, org, user, &input).await {
            Ok(_) => {
                imported_pages += 1;
                counters["pages"] = json!(imported_pages);
            }
            Err(e) => errors.push(format!("page {source_id}: {e}")),
        }
        update_progress(db, job_id, &counters, None).await;
    }

    // --- shelves (link imported books) ---
    counters["phase"] = json!("shelves");
    let mut imported_shelves = 0usize;
    for source in &source_shelves {
        let Some(source_id) = source.get("id").and_then(Value::as_i64) else { continue };
        let detail = match client.get_json(&format!("/api/shelves/{source_id}")).await {
            Ok(d) => d,
            Err(e) => {
                errors.push(format!("shelf {source_id}: {e}"));
                continue;
            }
        };
        let linked_books: Vec<i64> = detail
            .get("books")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|b| b.get("id").and_then(Value::as_i64))
                    .filter_map(|old| book_map.get(&old).copied())
                    .collect()
            })
            .unwrap_or_default();
        let input = shelves::CreateShelf {
            name: detail.get("name").and_then(Value::as_str).unwrap_or("Untitled").to_string(),
            description: detail.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
            books: linked_books,
            tags: tags_of(&detail),
        };
        match shelves::create(db, org, user, &input).await {
            Ok(_) => {
                imported_shelves += 1;
                counters["shelves"] = json!(imported_shelves);
            }
            Err(e) => errors.push(format!("shelf {source_id}: {e}")),
        }
        update_progress(db, job_id, &counters, None).await;
    }

    counters["phase"] = json!("done");
    errors.truncate(25);
    counters["errors"] = json!(errors);
    counters["note"] = json!("images, attachments and drafts are not imported");
    Ok(counters)
}
