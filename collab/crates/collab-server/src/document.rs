use crate::awareness::AwarenessState;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{sleep, Duration};
use yrs::Doc;

const GRACE_PERIOD_SECS: u64 = 30;

/// A live document handle shared across all connections for a given page.
pub struct DocHandle {
    pub doc: Arc<RwLock<Doc>>,
    pub update_tx: broadcast::Sender<Vec<u8>>,
    pub awareness_tx: broadcast::Sender<Vec<u8>>,
    pub awareness: Arc<RwLock<AwarenessState>>,
    pub connection_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl DocHandle {
    fn new() -> Self {
        let (update_tx, _) = broadcast::channel(256);
        let (awareness_tx, _) = broadcast::channel(256);
        Self {
            doc: Arc::new(RwLock::new(Doc::new())),
            update_tx,
            awareness_tx,
            awareness: Arc::new(RwLock::new(AwarenessState::default())),
            connection_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}

/// Central registry of active documents, keyed by document id (e.g. "page-42").
pub struct DocumentManager {
    docs: Arc<DashMap<String, Arc<DocHandle>>>,
}

impl DocumentManager {
    pub fn new() -> Self {
        Self {
            docs: Arc::new(DashMap::new()),
        }
    }

    /// Get or create a document handle for the given id.
    pub fn get_or_create(&self, doc_id: &str) -> Arc<DocHandle> {
        self.docs
            .entry(doc_id.to_string())
            .or_insert_with(DocHandle::new)
            .clone()
    }

    /// Called when a client disconnects from a document.
    /// If the connection count reaches 0, schedules removal after a grace period.
    pub fn on_disconnect(&self, doc_id: String) {
        let docs = Arc::clone(&self.docs);
        tokio::spawn(async move {
            sleep(Duration::from_secs(GRACE_PERIOD_SECS)).await;
            // Remove only if the entry still has 0 connections.
            docs.remove_if(&doc_id, |_, handle| {
                handle
                    .connection_count
                    .load(std::sync::atomic::Ordering::Relaxed)
                    == 0
            });
        });
    }

    /// List all active document ids.
    pub fn list_active(&self) -> Vec<String> {
        self.docs.iter().map(|e| e.key().clone()).collect()
    }

    /// Total active connection count across all documents.
    pub fn total_connections(&self) -> usize {
        self.docs
            .iter()
            .map(|e| {
                e.value()
                    .connection_count
                    .load(std::sync::atomic::Ordering::Relaxed)
            })
            .sum()
    }
}
