//! Semantic search engine: chunking, embedding (any OpenAI-compatible
//! embeddings API), an in-memory per-org vector cache backed by Postgres,
//! and a background indexer fed by `pg_notify` triggers — so every content
//! mutation path (REST, MCP, realtime collab persistence) keeps the index
//! fresh without explicit call sites.
//!
//! Search modes mirror bees-roadhouse/bookstack-mcp's `semantic_search`:
//! - `standard`: broad semantic sweep blended with keyword signals
//! - `precision`: tighter cascade favoring results that both embedding
//!   similarity and keyword search agree on (approximation of upstream's
//!   four-stage cascade; no cross-encoder rerank here)

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use sqlx::postgres::PgListener;
use sqlx::PgPool;
use tokio::sync::{mpsc, Mutex, RwLock};

use bookstack_core::services::search as keyword_search;
use bookstack_core::Core;

#[derive(Debug, thiserror::Error)]
pub enum SemanticError {
    #[error("semantic search is not configured (set EMBEDDINGS_API_URL)")]
    Disabled,
    #[error("embedding provider error: {0}")]
    Provider(String),
    #[error(transparent)]
    Core(#[from] bookstack_core::CoreError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, SemanticError>;

#[derive(Debug, Clone)]
pub struct SemanticConfig {
    pub api_url: String,
    pub api_key: Option<String>,
    pub model: String,
}

impl SemanticConfig {
    /// Enabled when EMBEDDINGS_API_URL is set. The URL should be the base of
    /// an OpenAI-compatible API (POST {url}/embeddings).
    pub fn from_env() -> Option<SemanticConfig> {
        let api_url = std::env::var("EMBEDDINGS_API_URL").ok()?;
        Some(SemanticConfig {
            api_url: api_url.trim_end_matches('/').to_string(),
            api_key: std::env::var("EMBEDDINGS_API_KEY").ok().filter(|k| !k.is_empty()),
            model: std::env::var("EMBEDDINGS_MODEL")
                .unwrap_or_else(|_| "text-embedding-3-small".to_string()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Standard,
    Precision,
}

impl Mode {
    pub fn parse(s: &str) -> Mode {
        match s {
            "precision" => Mode::Precision,
            _ => Mode::Standard, // "standard", "default", anything else
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Standard => "standard",
            Mode::Precision => "precision",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunkHit {
    pub content: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticResult {
    pub entity_type: String,
    pub org_id: i64,
    pub org_slug: String,
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub book_slug: Option<String>,
    pub score: f32,
    pub scoring: Value,
    pub chunks: Vec<ChunkHit>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub mode: &'static str,
    pub results: Vec<SemanticResult>,
    pub stats: Value,
}

#[derive(Debug, Serialize)]
pub struct Status {
    pub enabled: bool,
    pub model: String,
    pub embedded_chunks: i64,
    pub embedded_entities: i64,
    pub pending_jobs: usize,
}

#[derive(Clone)]
struct CachedChunk {
    entity_type: String,
    entity_id: i64,
    content: String,
    embedding: Arc<Vec<f32>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EntityKey {
    org_id: i64,
    entity_type: String,
    entity_id: i64,
}

pub struct SemanticEngine {
    core: Core,
    config: SemanticConfig,
    http: reqwest::Client,
    cache: RwLock<HashMap<i64, Vec<CachedChunk>>>,
    loaded_orgs: Mutex<HashSet<i64>>,
    queue: mpsc::UnboundedSender<EntityKey>,
    pending: AtomicUsize,
}

impl SemanticEngine {
    pub fn start(core: Core, config: SemanticConfig) -> Arc<SemanticEngine> {
        let (tx, rx) = mpsc::unbounded_channel();
        let engine = Arc::new(SemanticEngine {
            core,
            config,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
            cache: RwLock::new(HashMap::new()),
            loaded_orgs: Mutex::new(HashSet::new()),
            queue: tx,
            pending: AtomicUsize::new(0),
        });
        tokio::spawn(worker(engine.clone(), rx));
        tokio::spawn(listen_for_changes(engine.clone()));
        engine
    }

    pub fn model(&self) -> &str {
        &self.config.model
    }

    fn enqueue(&self, key: EntityKey) {
        self.pending.fetch_add(1, Ordering::SeqCst);
        let _ = self.queue.send(key);
    }

    /// Queue every live entity of an org for (re)embedding.
    pub async fn reembed_org(&self, org_id: i64) -> Result<usize> {
        let db = &self.core.db;
        let mut queued = 0usize;
        for (table, entity_type) in [
            ("shelves", "shelf"),
            ("books", "book"),
            ("chapters", "chapter"),
            ("pages", "page"),
        ] {
            let ids: Vec<(i64,)> = sqlx::query_as(&format!(
                "SELECT id FROM {table} WHERE org_id = $1 AND deleted_at IS NULL"
            ))
            .bind(org_id)
            .fetch_all(db)
            .await?;
            for (id,) in ids {
                self.enqueue(EntityKey {
                    org_id,
                    entity_type: entity_type.to_string(),
                    entity_id: id,
                });
                queued += 1;
            }
        }
        Ok(queued)
    }

    pub async fn status(&self, org_id: i64) -> Result<Status> {
        let (chunks, entities): (i64, i64) = sqlx::query_as(
            "SELECT count(*), count(DISTINCT (entity_type, entity_id))
             FROM embedding_chunks WHERE org_id = $1",
        )
        .bind(org_id)
        .fetch_one(&self.core.db)
        .await?;
        Ok(Status {
            enabled: true,
            model: self.config.model.clone(),
            embedded_chunks: chunks,
            embedded_entities: entities,
            pending_jobs: self.pending.load(Ordering::SeqCst),
        })
    }

    // ---- embedding provider ----

    async fn embed_batch(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(vec![]);
        }
        let mut request = self
            .http
            .post(format!("{}/embeddings", self.config.api_url))
            .json(&json!({ "model": self.config.model, "input": inputs }));
        if let Some(key) = &self.config.api_key {
            request = request.bearer_auth(key);
        }
        let response = request
            .send()
            .await
            .map_err(|e| SemanticError::Provider(e.to_string()))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(SemanticError::Provider(format!("{status}: {body}")));
        }
        let payload: Value = response
            .json()
            .await
            .map_err(|e| SemanticError::Provider(format!("invalid JSON: {e}")))?;
        let data = payload
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| SemanticError::Provider("response missing data[]".into()))?;
        let mut out = Vec::with_capacity(data.len());
        for item in data {
            let embedding = item
                .get("embedding")
                .and_then(Value::as_array)
                .ok_or_else(|| SemanticError::Provider("missing embedding".into()))?
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect::<Vec<f32>>();
            out.push(normalize(embedding));
        }
        Ok(out)
    }

    // ---- indexing ----

    async fn index_entity(&self, key: &EntityKey) -> Result<()> {
        let db = &self.core.db;
        let source: Option<(String, String)> = match key.entity_type.as_str() {
            "page" => sqlx::query_as(
                "SELECT name, markdown FROM pages WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL AND draft = false",
            )
            .bind(key.entity_id)
            .bind(key.org_id)
            .fetch_optional(db)
            .await?,
            "book" => sqlx::query_as(
                "SELECT name, description FROM books WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL",
            )
            .bind(key.entity_id)
            .bind(key.org_id)
            .fetch_optional(db)
            .await?,
            "chapter" => sqlx::query_as(
                "SELECT name, description FROM chapters WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL",
            )
            .bind(key.entity_id)
            .bind(key.org_id)
            .fetch_optional(db)
            .await?,
            _ => sqlx::query_as(
                "SELECT name, description FROM shelves WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL",
            )
            .bind(key.entity_id)
            .bind(key.org_id)
            .fetch_optional(db)
            .await?,
        };

        let Some((name, body)) = source else {
            // Deleted or missing: drop its chunks.
            return self.remove_entity(key).await;
        };

        let chunks = chunk_content(&name, &body);
        if chunks.is_empty() {
            return self.remove_entity(key).await;
        }
        let embeddings = self.embed_batch(&chunks).await?;

        let mut tx = db.begin().await?;
        sqlx::query("DELETE FROM embedding_chunks WHERE entity_type = $1 AND entity_id = $2")
            .bind(&key.entity_type)
            .bind(key.entity_id)
            .execute(&mut *tx)
            .await?;
        for (index, (content, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
            sqlx::query(
                "INSERT INTO embedding_chunks (org_id, entity_type, entity_id, chunk_index, content, embedding, model)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(key.org_id)
            .bind(&key.entity_type)
            .bind(key.entity_id)
            .bind(index as i32)
            .bind(content)
            .bind(embedding_bytes(embedding))
            .bind(&self.config.model)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        // Refresh the org's cache entries for this entity.
        let mut cache = self.cache.write().await;
        let org_chunks = cache.entry(key.org_id).or_default();
        org_chunks.retain(|c| !(c.entity_type == key.entity_type && c.entity_id == key.entity_id));
        for (content, embedding) in chunks.into_iter().zip(embeddings.into_iter()) {
            org_chunks.push(CachedChunk {
                entity_type: key.entity_type.clone(),
                entity_id: key.entity_id,
                content,
                embedding: Arc::new(embedding),
            });
        }
        Ok(())
    }

    async fn remove_entity(&self, key: &EntityKey) -> Result<()> {
        sqlx::query("DELETE FROM embedding_chunks WHERE entity_type = $1 AND entity_id = $2")
            .bind(&key.entity_type)
            .bind(key.entity_id)
            .execute(&self.core.db)
            .await?;
        let mut cache = self.cache.write().await;
        if let Some(org_chunks) = cache.get_mut(&key.org_id) {
            org_chunks.retain(|c| !(c.entity_type == key.entity_type && c.entity_id == key.entity_id));
        }
        Ok(())
    }

    async fn ensure_org_loaded(&self, org_id: i64) -> Result<()> {
        {
            let loaded = self.loaded_orgs.lock().await;
            if loaded.contains(&org_id) {
                return Ok(());
            }
        }
        let rows: Vec<(String, i64, String, Vec<u8>)> = sqlx::query_as(
            "SELECT entity_type, entity_id, content, embedding FROM embedding_chunks WHERE org_id = $1",
        )
        .bind(org_id)
        .fetch_all(&self.core.db)
        .await?;
        let mut cache = self.cache.write().await;
        let org_chunks = cache.entry(org_id).or_default();
        org_chunks.clear();
        for (entity_type, entity_id, content, bytes) in rows {
            org_chunks.push(CachedChunk {
                entity_type,
                entity_id,
                content,
                embedding: Arc::new(bytes_to_embedding(&bytes)),
            });
        }
        self.loaded_orgs.lock().await.insert(org_id);
        Ok(())
    }

    // ---- search ----

    pub async fn search(
        &self,
        org_ids: &[i64],
        query: &str,
        mode: Mode,
        count: usize,
    ) -> Result<SearchResponse> {
        let started = std::time::Instant::now();
        let query = query.trim();
        if query.is_empty() || org_ids.is_empty() {
            return Ok(SearchResponse {
                mode: mode.as_str(),
                results: vec![],
                stats: json!({ "semantic_ms": 0 }),
            });
        }
        let query_embedding = self
            .embed_batch(&[query.to_string()])
            .await?
            .pop()
            .ok_or_else(|| SemanticError::Provider("empty embedding response".into()))?;

        // Cosine over the in-memory cache (embeddings are normalized).
        let mut per_entity: HashMap<EntityKey, (f32, Vec<ChunkHit>)> = HashMap::new();
        for &org_id in org_ids {
            self.ensure_org_loaded(org_id).await?;
            let cache = self.cache.read().await;
            let Some(org_chunks) = cache.get(&org_id) else { continue };
            for chunk in org_chunks {
                let score = dot(&query_embedding, &chunk.embedding);
                let key = EntityKey {
                    org_id,
                    entity_type: chunk.entity_type.clone(),
                    entity_id: chunk.entity_id,
                };
                let entry = per_entity.entry(key).or_insert_with(|| (f32::MIN, vec![]));
                entry.0 = entry.0.max(score);
                entry.1.push(ChunkHit {
                    content: excerpt(&chunk.content, 280),
                    score,
                });
            }
        }

        let semantic_ms = started.elapsed().as_millis();

        // Keyword pass (shared FTS) for blending.
        let keyword_started = std::time::Instant::now();
        let keyword = keyword_search::search(&self.core.db, org_ids, query, &[], 50, 0, None)
            .await
            .unwrap_or(bookstack_core::models::Paginated { data: vec![], total: 0 });
        let keyword_ms = keyword_started.elapsed().as_millis();
        let max_keyword_rank = keyword.data.iter().map(|r| r.rank).fold(0.0_f32, f32::max).max(1e-6);
        let keyword_scores: HashMap<(String, i64), f32> = keyword
            .data
            .iter()
            .map(|r| ((r.entity_type.clone(), r.id), r.rank / max_keyword_rank))
            .collect();

        // Blend semantic + keyword per mode.
        let mut scored: Vec<(EntityKey, f32, Value, Vec<ChunkHit>)> = per_entity
            .into_iter()
            .map(|(key, (semantic, mut chunks))| {
                chunks.sort_by(|a, b| b.score.total_cmp(&a.score));
                chunks.truncate(3);
                let keyword_score = keyword_scores
                    .get(&(key.entity_type.clone(), key.entity_id))
                    .copied()
                    .unwrap_or(0.0);
                let (combined, scoring) = match mode {
                    Mode::Standard => {
                        let combined = 0.8 * semantic + 0.2 * keyword_score;
                        (combined, json!({ "semantic": semantic, "keyword": keyword_score }))
                    }
                    Mode::Precision => {
                        let agreement = if keyword_score > 0.0 { 1.25 } else { 1.0 };
                        let combined = (0.65 * semantic + 0.35 * keyword_score) * agreement;
                        (
                            combined,
                            json!({
                                "semantic": semantic,
                                "keyword": keyword_score,
                                "agreement_boost": agreement,
                            }),
                        )
                    }
                };
                (key, combined, scoring, chunks)
            })
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));

        // Precision keeps only confident hits.
        if mode == Mode::Precision {
            if let Some(top) = scored.first().map(|(_, s, _, _)| *s) {
                scored.retain(|(_, s, _, _)| *s >= top * 0.6);
            }
        }
        scored.truncate(count.clamp(1, 50));

        // Decorate with entity metadata.
        let mut results = Vec::with_capacity(scored.len());
        for (key, score, scoring, chunks) in scored {
            if let Some(meta) = self.entity_meta(&key).await? {
                let (name, slug, book_slug, org_slug) = meta;
                results.push(SemanticResult {
                    entity_type: key.entity_type,
                    org_id: key.org_id,
                    org_slug,
                    id: key.entity_id,
                    name,
                    slug,
                    book_slug,
                    score,
                    scoring,
                    chunks,
                });
            }
        }

        Ok(SearchResponse {
            mode: mode.as_str(),
            results,
            stats: json!({
                "semantic_ms": semantic_ms,
                "keyword_ms": keyword_ms,
                "model": self.config.model,
            }),
        })
    }

    async fn entity_meta(
        &self,
        key: &EntityKey,
    ) -> Result<Option<(String, String, Option<String>, String)>> {
        let db = &self.core.db;
        let row: Option<(String, String, Option<String>, String)> = match key.entity_type.as_str() {
            "page" => sqlx::query_as(
                "SELECT p.name, p.slug, b.slug, o.slug FROM pages p
                 JOIN books b ON b.id = p.book_id JOIN orgs o ON o.id = p.org_id
                 WHERE p.id = $1 AND p.deleted_at IS NULL",
            )
            .bind(key.entity_id)
            .fetch_optional(db)
            .await?,
            "chapter" => sqlx::query_as(
                "SELECT c.name, c.slug, b.slug, o.slug FROM chapters c
                 JOIN books b ON b.id = c.book_id JOIN orgs o ON o.id = c.org_id
                 WHERE c.id = $1 AND c.deleted_at IS NULL",
            )
            .bind(key.entity_id)
            .fetch_optional(db)
            .await?,
            "book" => sqlx::query_as(
                "SELECT b.name, b.slug, NULL, o.slug FROM books b
                 JOIN orgs o ON o.id = b.org_id WHERE b.id = $1 AND b.deleted_at IS NULL",
            )
            .bind(key.entity_id)
            .fetch_optional(db)
            .await?,
            _ => sqlx::query_as(
                "SELECT s.name, s.slug, NULL, o.slug FROM shelves s
                 JOIN orgs o ON o.id = s.org_id WHERE s.id = $1 AND s.deleted_at IS NULL",
            )
            .bind(key.entity_id)
            .fetch_optional(db)
            .await?,
        };
        Ok(row)
    }
}

/// Debounced worker: coalesces bursts (e.g. collab persistence every 4s)
/// before hitting the embedding provider.
async fn worker(engine: Arc<SemanticEngine>, mut rx: mpsc::UnboundedReceiver<EntityKey>) {
    loop {
        let Some(first) = rx.recv().await else { break };
        let mut batch: HashSet<EntityKey> = HashSet::new();
        batch.insert(first);
        // Quiet period to coalesce rapid successive updates to the same entity.
        loop {
            match tokio::time::timeout(Duration::from_millis(1500), rx.recv()).await {
                Ok(Some(key)) => {
                    batch.insert(key);
                    if batch.len() >= 32 {
                        break;
                    }
                }
                Ok(None) => return,
                Err(_) => break,
            }
        }
        for key in batch {
            if let Err(err) = engine.index_entity(&key).await {
                tracing::warn!(
                    entity = %format!("{}:{}", key.entity_type, key.entity_id),
                    "semantic indexing failed: {err}"
                );
            }
            engine.pending.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// LISTEN for content-change notifications emitted by the DB triggers.
async fn listen_for_changes(engine: Arc<SemanticEngine>) {
    loop {
        match PgListener::connect_with(pool(&engine.core)).await {
            Ok(mut listener) => {
                if listener.listen("bookstack_content").await.is_err() {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                tracing::info!("semantic indexer listening for content changes");
                while let Ok(notification) = listener.recv().await {
                    if let Ok(payload) = serde_json::from_str::<Value>(notification.payload()) {
                        let entity_type = payload
                            .get("entity_type")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let entity_id = payload.get("entity_id").and_then(Value::as_i64).unwrap_or(0);
                        let org_id = payload.get("org_id").and_then(Value::as_i64).unwrap_or(0);
                        if entity_id > 0 && org_id > 0 && !entity_type.is_empty() {
                            engine.enqueue(EntityKey { org_id, entity_type, entity_id });
                        }
                    }
                }
                tracing::warn!("semantic listener disconnected; reconnecting");
            }
            Err(err) => {
                tracing::warn!("semantic listener connect failed: {err}");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

fn pool(core: &Core) -> &PgPool {
    &core.db
}

// ---- chunking + vector helpers ----

/// Heading-aware chunking: split on markdown headings, then pack into
/// ~1200-char chunks with the entity name prefixed for context.
fn chunk_content(name: &str, body: &str) -> Vec<String> {
    const TARGET: usize = 1200;
    let mut sections: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in body.lines() {
        if line.starts_with('#') && !current.trim().is_empty() {
            sections.push(current.clone());
            current.clear();
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.trim().is_empty() {
        sections.push(current);
    }
    if sections.is_empty() && !name.trim().is_empty() {
        sections.push(String::new());
    }

    let mut chunks = Vec::new();
    let mut packed = String::new();
    for section in sections {
        if !packed.is_empty() && packed.len() + section.len() > TARGET {
            chunks.push(format!("{name}\n\n{}", packed.trim()));
            packed.clear();
        }
        if section.len() > TARGET {
            // Hard-split oversized sections on char boundaries.
            let mut start = 0;
            let chars: Vec<char> = section.chars().collect();
            while start < chars.len() {
                let end = (start + TARGET).min(chars.len());
                let piece: String = chars[start..end].iter().collect();
                chunks.push(format!("{name}\n\n{}", piece.trim()));
                start = end;
            }
        } else {
            packed.push_str(&section);
        }
    }
    if !packed.trim().is_empty() || chunks.is_empty() {
        chunks.push(format!("{name}\n\n{}", packed.trim()));
    }
    chunks.retain(|c| !c.trim().is_empty());
    chunks
}

fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn embedding_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn excerpt(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut: String = text.chars().take(max_chars).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunking_splits_headings() {
        let chunks = chunk_content("Page", "intro\n\n## A\naaa\n\n## B\nbbb");
        assert!(!chunks.is_empty());
        assert!(chunks[0].starts_with("Page\n\n"));
    }

    #[test]
    fn vector_roundtrip() {
        let v = normalize(vec![3.0, 4.0]);
        let bytes = embedding_bytes(&v);
        let back = bytes_to_embedding(&bytes);
        assert_eq!(v, back);
        assert!((dot(&v, &back) - 1.0).abs() < 1e-6);
    }
}
