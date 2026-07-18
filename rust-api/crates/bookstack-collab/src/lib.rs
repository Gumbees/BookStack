//! Realtime collaborative editing engine.
//!
//! Each actively-edited page gets a [`Room`] holding a shared Yjs (yrs) CRDT
//! document. Clients connect over WebSocket speaking the standard
//! `y-websocket` binary protocol (sync + awareness messages), so the stock
//! `y-websocket` / `y-codemirror.next` browser stack works unmodified.
//!
//! The room's document is seeded from the page's persisted CRDT state (or its
//! markdown when no CRDT state exists yet), and flattened back to
//! markdown/HTML in Postgres on a debounce timer and when the last editor
//! leaves.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::{broadcast, watch, Mutex};
use yrs::sync::{Awareness, Message, SyncMessage};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, Update};

use bookstack_core::services::pages;
use bookstack_core::{Core, CoreError};

/// Name of the shared text root in every page document. The frontend must
/// bind its editor to `doc.getText(TEXT_ROOT)`.
pub const TEXT_ROOT: &str = "content";

const PERSIST_INTERVAL: Duration = Duration::from_secs(4);
const BROADCAST_CAPACITY: usize = 512;

#[derive(Debug, thiserror::Error)]
pub enum CollabError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("room is closed")]
    Closed,
}

pub type Result<T> = std::result::Result<T, CollabError>;

pub struct Room {
    pub page_id: i64,
    awareness: Mutex<Awareness>,
    broadcast: broadcast::Sender<Vec<u8>>,
    shutdown: watch::Sender<bool>,
    dirty: AtomicBool,
    connections: AtomicUsize,
    closed: AtomicBool,
    _doc_sub: yrs::Subscription,
}

impl Room {
    async fn create(core: &Core, page_id: i64) -> Result<Arc<Room>> {
        let source = pages::collab_source(&core.db, page_id).await?;
        let doc = Doc::new();
        let text = doc.get_or_insert_text(TEXT_ROOT);

        match source.ydoc_state {
            Some(state) => {
                let update = Update::decode_v1(&state)
                    .map_err(|e| CollabError::Protocol(format!("stored doc state: {e}")))?;
                let mut txn = doc.transact_mut();
                txn.apply_update(update)
                    .map_err(|e| CollabError::Protocol(format!("stored doc state: {e}")))?;
            }
            None => {
                if !source.markdown.is_empty() {
                    let mut txn = doc.transact_mut();
                    text.insert(&mut txn, 0, &source.markdown);
                }
            }
        }

        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (shutdown_tx, _) = watch::channel(false);

        let broadcast_tx = tx.clone();
        let doc_sub = doc
            .observe_update_v1(move |_txn, event| {
                let frame = Message::Sync(SyncMessage::Update(event.update.clone())).encode_v1();
                let _ = broadcast_tx.send(frame);
            })
            .expect("observe_update_v1");

        Ok(Arc::new(Room {
            page_id,
            awareness: Mutex::new(Awareness::new(doc)),
            broadcast: tx,
            shutdown: shutdown_tx,
            dirty: AtomicBool::new(false),
            connections: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
            _doc_sub: doc_sub,
        }))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.broadcast.subscribe()
    }

    pub fn shutdown_signal(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }

    pub fn active_connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// Frames the server sends to a client immediately after it connects:
    /// sync step 1 (our state vector) plus current awareness states.
    pub async fn connect_frames(&self) -> Vec<Vec<u8>> {
        let awareness = self.awareness.lock().await;
        let mut frames = Vec::with_capacity(2);
        {
            let txn = awareness.doc().transact();
            frames.push(Message::Sync(SyncMessage::SyncStep1(txn.state_vector())).encode_v1());
        }
        if let Ok(update) = awareness.update() {
            if !update.clients.is_empty() {
                frames.push(Message::Awareness(update).encode_v1());
            }
        }
        frames
    }

    /// Handle one inbound client frame. Returns direct replies for that
    /// client; document/awareness changes are broadcast to the room via the
    /// update observer and explicit sends here.
    pub async fn handle_message(
        &self,
        data: &[u8],
        seen_clients: &mut HashSet<u64>,
    ) -> Result<Vec<Vec<u8>>> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(CollabError::Closed);
        }
        let message = Message::decode_v1(data)
            .map_err(|e| CollabError::Protocol(format!("bad frame: {e}")))?;
        let mut replies = Vec::new();
        let awareness = self.awareness.lock().await;

        match message {
            Message::Sync(SyncMessage::SyncStep1(state_vector)) => {
                let txn = awareness.doc().transact();
                let diff = txn.encode_state_as_update_v1(&state_vector);
                replies.push(Message::Sync(SyncMessage::SyncStep2(diff)).encode_v1());
            }
            Message::Sync(SyncMessage::SyncStep2(update) | SyncMessage::Update(update)) => {
                let update = Update::decode_v1(&update)
                    .map_err(|e| CollabError::Protocol(format!("bad update: {e}")))?;
                let mut txn = awareness.doc().transact_mut();
                txn.apply_update(update)
                    .map_err(|e| CollabError::Protocol(format!("apply update: {e}")))?;
                drop(txn);
                self.dirty.store(true, Ordering::SeqCst);
            }
            Message::Awareness(update) => {
                for client_id in update.clients.keys() {
                    seen_clients.insert(*client_id);
                }
                awareness
                    .apply_update(update)
                    .map_err(|e| CollabError::Protocol(format!("awareness: {e}")))?;
                // Relay the original frame to everyone in the room.
                let _ = self.broadcast.send(data.to_vec());
            }
            Message::AwarenessQuery => {
                if let Ok(update) = awareness.update() {
                    replies.push(Message::Awareness(update).encode_v1());
                }
            }
            Message::Auth(_) | Message::Custom(_, _) => {}
        }
        Ok(replies)
    }

    /// Remove a departing connection's awareness presences and notify peers.
    async fn clear_clients(&self, seen_clients: &HashSet<u64>) {
        if seen_clients.is_empty() {
            return;
        }
        let mut awareness = self.awareness.lock().await;
        for client_id in seen_clients {
            awareness.remove_state(*client_id);
        }
        let ids: Vec<u64> = seen_clients.iter().copied().collect();
        if let Ok(update) = awareness.update_with_clients(ids) {
            let _ = self.broadcast.send(Message::Awareness(update).encode_v1());
        }
    }

    /// Flatten the CRDT document to (markdown, full state update).
    async fn flatten(&self) -> (String, Vec<u8>) {
        let awareness = self.awareness.lock().await;
        let doc = awareness.doc();
        let text = doc.get_or_insert_text(TEXT_ROOT);
        let txn = doc.transact();
        let markdown = text.get_string(&txn);
        let state = txn.encode_state_as_update_v1(&StateVector::default());
        (markdown, state)
    }

    async fn persist(&self, core: &Core) -> Result<()> {
        let (markdown, state) = self.flatten().await;
        pages::persist_collab(&core.db, self.page_id, &markdown, &state).await?;
        Ok(())
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        let _ = self.shutdown.send(true);
    }
}

/// Registry of live rooms plus persistence loops.
pub struct CollabEngine {
    core: Core,
    rooms: DashMap<i64, Arc<Room>>,
    create_lock: Mutex<()>,
}

/// One client's membership in a room. Call [`CollabEngine::leave`] when the
/// socket closes.
pub struct Session {
    pub room: Arc<Room>,
    pub seen_clients: HashSet<u64>,
}

impl CollabEngine {
    pub fn new(core: Core) -> Arc<CollabEngine> {
        let engine = Arc::new(CollabEngine {
            core,
            rooms: DashMap::new(),
            create_lock: Mutex::new(()),
        });
        // Periodic persistence sweep for dirty rooms.
        let sweep = Arc::downgrade(&engine);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(PERSIST_INTERVAL);
            loop {
                ticker.tick().await;
                let Some(engine) = sweep.upgrade() else { break };
                let rooms: Vec<Arc<Room>> =
                    engine.rooms.iter().map(|entry| entry.value().clone()).collect();
                for room in rooms {
                    if room.dirty.swap(false, Ordering::SeqCst) {
                        if let Err(err) = room.persist(&engine.core).await {
                            tracing::error!(page = room.page_id, "collab persist failed: {err}");
                            room.dirty.store(true, Ordering::SeqCst);
                        }
                    }
                }
            }
        });
        engine
    }

    /// Join (or create) the room for a page.
    pub async fn join(&self, page_id: i64) -> Result<Session> {
        let _guard = self.create_lock.lock().await;
        let room = match self.rooms.get(&page_id) {
            Some(existing) if !existing.closed.load(Ordering::SeqCst) => existing.clone(),
            _ => {
                self.rooms.remove(&page_id);
                let room = Room::create(&self.core, page_id).await?;
                self.rooms.insert(page_id, room.clone());
                room
            }
        };
        room.connections.fetch_add(1, Ordering::SeqCst);
        Ok(Session { room, seen_clients: HashSet::new() })
    }

    /// Leave a room. When the last editor disconnects the document is
    /// persisted, a revision snapshot is recorded, and the room is dropped.
    pub async fn leave(&self, session: Session, user_id: Option<i64>) {
        let Session { room, seen_clients } = session;
        room.clear_clients(&seen_clients).await;
        let remaining = room.connections.fetch_sub(1, Ordering::SeqCst) - 1;
        if remaining > 0 {
            return;
        }
        let _guard = self.create_lock.lock().await;
        if room.connections.load(Ordering::SeqCst) > 0 || room.closed.load(Ordering::SeqCst) {
            return;
        }
        room.close();
        self.rooms.remove(&room.page_id);
        if let Err(err) = room.persist(&self.core).await {
            tracing::error!(page = room.page_id, "final collab persist failed: {err}");
            return;
        }
        if let Err(err) = pages::snapshot_revision(
            &self.core.db,
            room.page_id,
            user_id,
            "Collaborative editing session",
        )
        .await
        {
            tracing::warn!(page = room.page_id, "collab revision snapshot failed: {err}");
        }
    }

    /// Persist a room immediately (explicit "save now"). Returns false when
    /// the page has no live room.
    pub async fn flush(&self, page_id: i64, user_id: Option<i64>) -> Result<bool> {
        let Some(room) = self.rooms.get(&page_id).map(|r| r.value().clone()) else {
            return Ok(false);
        };
        room.dirty.store(false, Ordering::SeqCst);
        room.persist(&self.core).await?;
        pages::snapshot_revision(&self.core.db, page_id, user_id, "Manual save during collaboration")
            .await?;
        Ok(true)
    }

    /// Force-close a page's room without persisting, discarding in-memory CRDT
    /// state. Used when page content is replaced through the REST/MCP API so
    /// editors reconnect against the new content.
    pub async fn invalidate(&self, page_id: i64) {
        let _guard = self.create_lock.lock().await;
        if let Some((_, room)) = self.rooms.remove(&page_id) {
            room.close();
        }
    }

    /// Number of live editing connections for a page.
    pub fn active_editors(&self, page_id: i64) -> usize {
        self.rooms
            .get(&page_id)
            .map(|room| room.active_connections())
            .unwrap_or(0)
    }

    /// Persist every dirty room (graceful shutdown).
    pub async fn flush_all(&self) {
        let rooms: Vec<Arc<Room>> = self.rooms.iter().map(|entry| entry.value().clone()).collect();
        for room in rooms {
            if room.dirty.swap(false, Ordering::SeqCst) {
                if let Err(err) = room.persist(&self.core).await {
                    tracing::error!(page = room.page_id, "shutdown persist failed: {err}");
                }
            }
        }
    }
}
