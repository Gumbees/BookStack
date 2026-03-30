use crate::{
    auth::{verify_token, TokenClaims},
    awareness::UserState,
    config::Config,
    document::DocumentManager,
};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, info, warn};
use yrs::{
    updates::{decoder::Decode, encoder::Encode},
    ReadTxn, StateVector, Transact, Update,
};

// Yrs sync protocol message types
const MSG_SYNC_STEP1: u8 = 0;
const MSG_SYNC_STEP2: u8 = 1;
const MSG_UPDATE: u8 = 2;
const MSG_AWARENESS: u8 = 3;

#[derive(Deserialize)]
pub struct WsParams {
    token: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::Path(doc_id): axum::extract::Path<String>,
    Query(params): Query<WsParams>,
    State((config, doc_mgr)): State<(Arc<Config>, Arc<DocumentManager>)>,
) -> impl IntoResponse {
    let claims = match verify_token(&params.token, &config.jwt_secret) {
        Ok(c) => c,
        Err(e) => {
            warn!("WebSocket auth rejected: {e}");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    // Validate that the doc_id matches the page from the token (page-{pageId}).
    let expected_doc = format!("page-{}", claims.page_id);
    if doc_id != expected_doc {
        warn!(
            "Token page_id {} does not match doc_id {}",
            claims.page_id, doc_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }

    let handle = doc_mgr.get_or_create(&doc_id);
    ws.on_upgrade(move |socket| handle_socket(socket, doc_id, claims, handle, doc_mgr))
}

async fn handle_socket(
    socket: WebSocket,
    doc_id: String,
    claims: TokenClaims,
    handle: Arc<crate::document::DocHandle>,
    doc_mgr: Arc<DocumentManager>,
) {
    let client_id = uuid::Uuid::new_v4().to_string();
    handle
        .connection_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    info!(
        "Client {} ({}) connected to {}",
        client_id, claims.user_name, doc_id
    );

    // Register user presence.
    {
        let mut awareness = handle.awareness.write().await;
        awareness.upsert(
            client_id.clone(),
            UserState::new(claims.user_id, claims.user_name.clone()),
        );
    }
    broadcast_awareness(&handle).await;

    let mut update_rx = handle.update_tx.subscribe();
    let mut awareness_rx = handle.awareness_tx.subscribe();

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Send sync step 1: our current state vector so the client can send us missing updates.
    // All yrs types are scoped and dropped before any .await to satisfy Send bounds.
    let sync_step1_msg = {
        let doc = handle.doc.read().await;
        let sv_bytes = doc.transact().state_vector().encode_v1();
        drop(doc);
        let mut msg = vec![MSG_SYNC_STEP1];
        msg.extend_from_slice(&encode_var_uint(sv_bytes.len()));
        msg.extend_from_slice(&sv_bytes);
        msg
    };
    if ws_tx.send(Message::Binary(sync_step1_msg.into())).await.is_err() {
        cleanup(client_id, doc_id, handle, doc_mgr).await;
        return;
    }

    loop {
        tokio::select! {
            // Inbound message from this client.
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Binary(data))) => {
                        if data.is_empty() {
                            continue;
                        }
                        match data[0] {
                            MSG_SYNC_STEP1 => {
                                // Client sent its state vector; respond with step 2 (missing updates).
                                // Scope yrs types before any .await to satisfy Send bounds.
                                let resp = if let Some(sv_bytes) = decode_prefixed(&data[1..]) {
                                    let doc = handle.doc.read().await;
                                    let result = if let Ok(sv) = StateVector::decode_v1(&sv_bytes) {
                                        let diff = doc.transact().encode_diff_v1(&sv);
                                        let mut msg = vec![MSG_SYNC_STEP2];
                                        msg.extend_from_slice(&encode_var_uint(diff.len()));
                                        msg.extend_from_slice(&diff);
                                        Some(msg)
                                    } else {
                                        None
                                    };
                                    drop(doc);
                                    result
                                } else {
                                    None
                                };
                                if let Some(msg) = resp {
                                    let _ = ws_tx.send(Message::Binary(msg.into())).await;
                                }
                            }
                            MSG_SYNC_STEP2 | MSG_UPDATE => {
                                // Client sent an update; apply it and broadcast to others.
                                // Decode and apply synchronously, then broadcast.
                                let broadcast_msg = if let Some(update_bytes) = decode_prefixed(&data[1..]) {
                                    let mut doc = handle.doc.write().await;
                                    let applied = if let Ok(update) = Update::decode_v1(&update_bytes) {
                                        let _ = doc.transact_mut().apply_update(update);
                                        true
                                    } else {
                                        false
                                    };
                                    drop(doc);
                                    if applied {
                                        let mut msg = vec![MSG_UPDATE];
                                        msg.extend_from_slice(&encode_var_uint(update_bytes.len()));
                                        msg.extend_from_slice(&update_bytes);
                                        Some(msg)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };
                                if let Some(msg) = broadcast_msg {
                                    let _ = handle.update_tx.send(msg);
                                }
                            }
                            MSG_AWARENESS => {
                                // Client sent awareness (cursor/selection update).
                                if let Some(payload) = decode_prefixed(&data[1..]) {
                                    if let Ok(state) = serde_json::from_slice::<AwarenessUpdate>(&payload) {
                                        let mut awareness = handle.awareness.write().await;
                                        if let Some(user) = awareness.users.get_mut(&client_id) {
                                            user.cursor_pos = state.cursor_pos;
                                            user.selection_start = state.selection_start;
                                            user.selection_end = state.selection_end;
                                        }
                                    }
                                    broadcast_awareness(&handle).await;
                                }
                            }
                            _ => {
                                debug!("Unknown message type {} from {}", data[0], client_id);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        debug!("WebSocket error from {}: {e}", client_id);
                        break;
                    }
                    _ => {}
                }
            }

            // Outbound: relay updates from other clients.
            Ok(msg) = update_rx.recv() => {
                if ws_tx.send(Message::Binary(msg.into())).await.is_err() {
                    break;
                }
            }

            // Outbound: relay awareness broadcasts.
            Ok(msg) = awareness_rx.recv() => {
                if ws_tx.send(Message::Binary(msg.into())).await.is_err() {
                    break;
                }
            }
        }
    }

    cleanup(client_id, doc_id, handle, doc_mgr).await;
}

async fn cleanup(
    client_id: String,
    doc_id: String,
    handle: Arc<crate::document::DocHandle>,
    doc_mgr: Arc<DocumentManager>,
) {
    handle
        .connection_count
        .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

    {
        let mut awareness = handle.awareness.write().await;
        awareness.remove(&client_id);
    }
    broadcast_awareness(&handle).await;

    info!("Client {} disconnected from {}", client_id, doc_id);
    doc_mgr.on_disconnect(doc_id);
}

async fn broadcast_awareness(handle: &crate::document::DocHandle) {
    let awareness = handle.awareness.read().await;
    let snapshot = awareness.snapshot();
    if let Ok(payload) = serde_json::to_vec(&snapshot) {
        let mut msg = vec![MSG_AWARENESS];
        msg.extend_from_slice(&encode_var_uint(payload.len()));
        msg.extend_from_slice(&payload);
        let _ = handle.awareness_tx.send(msg);
    }
}

/// Decode a length-prefixed byte slice (variable-length uint prefix).
fn decode_prefixed(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() {
        return None;
    }
    let (len, consumed) = decode_var_uint(data)?;
    let start = consumed;
    let end = start + len;
    if end > data.len() {
        return None;
    }
    Some(data[start..end].to_vec())
}

/// Encode a usize as a variable-length uint (little-endian 7-bit encoding).
fn encode_var_uint(mut n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            break;
        } else {
            out.push(byte | 0x80);
        }
    }
    out
}

/// Decode a variable-length uint. Returns (value, bytes_consumed).
fn decode_var_uint(data: &[u8]) -> Option<(usize, usize)> {
    let mut result: usize = 0;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate() {
        result |= ((byte & 0x7f) as usize) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
        if shift >= 64 {
            return None;
        }
    }
    None
}

#[derive(serde::Deserialize)]
struct AwarenessUpdate {
    cursor_pos: u32,
    selection_start: u32,
    selection_end: u32,
}
