use std::collections::HashMap;

use axum::body::Bytes;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};

use bookstack_collab::{CollabError, CLOSE_CONTENT_REPLACED};
use bookstack_core::models::AuthUser;
use bookstack_core::services::{orgs, pages};
use bookstack_core::CoreError;

use crate::error::{ApiError, ApiResult};
use crate::extract::resolve_bare_token;
use crate::state::AppState;

/// Collaborative editing WebSocket. Speaks the standard `y-websocket` binary
/// protocol; authenticate with `?token=<jwt-or-api-token>`. The caller must
/// hold the editor role inside the page's org.
pub async fn page_ws(
    State(state): State<AppState>,
    Path(page_id): Path<i64>,
    Query(query): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> ApiResult<Response> {
    let token = query.get("token").ok_or(ApiError(CoreError::Unauthorized))?;
    let resolved = resolve_bare_token(&state.core, token).await?;
    // The page's own org decides access, regardless of the client's active org.
    let source = pages::collab_source(&state.core.db, page_id).await?;
    let role = orgs::role_in(
        &state.core.db,
        resolved.user.id,
        source.org_id,
        resolved.user.role.is_admin(),
    )
    .await?;
    if !role.can_edit() {
        return Err(ApiError(CoreError::Forbidden));
    }
    let user = resolved.user;

    Ok(ws.on_upgrade(move |socket| handle_socket(state, socket, page_id, user)))
}

fn content_replaced_close() -> Message {
    Message::Close(Some(CloseFrame {
        code: CLOSE_CONTENT_REPLACED,
        reason: "content-replaced".into(),
    }))
}

/// Drain inbound frames until the peer acknowledges the close (or times
/// out). Dropping the socket with unread data would RST the connection and
/// destroy the close frame we just queued — the client would see 1006
/// instead of our close code.
async fn await_close_ack<S>(stream: &mut S)
where
    S: StreamExt<Item = Result<Message, axum::Error>> + Unpin,
{
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Some(message) = stream.next().await {
            if matches!(message, Ok(Message::Close(_)) | Err(_)) {
                break;
            }
        }
    })
    .await;
}

async fn handle_socket(state: AppState, mut socket: WebSocket, page_id: i64, user: AuthUser) {
    let mut session = match state.collab.join(page_id).await {
        Ok(session) => session,
        Err(CollabError::RecentlyInvalidated) => {
            // Tell the client its local document is stale; it must rebuild
            // before rejoining (see CLOSE_CONTENT_REPLACED docs).
            let _ = socket.send(content_replaced_close()).await;
            await_close_ack(&mut socket).await;
            return;
        }
        Err(err) => {
            tracing::warn!(page = page_id, "collab join failed: {err}");
            return;
        }
    };

    let mut room_rx = session.room.subscribe();
    let mut shutdown_rx = session.room.shutdown_signal();
    let (mut sink, mut stream) = socket.split();

    // Initial handshake: sync step 1 + current awareness states.
    for frame in session.room.connect_frames().await {
        if sink.send(Message::Binary(Bytes::from(frame))).await.is_err() {
            state.collab.leave(session, Some(user.id)).await;
            return;
        }
    }

    loop {
        tokio::select! {
            inbound = stream.next() => {
                match inbound {
                    Some(Ok(Message::Binary(data))) => {
                        match session.room.clone().handle_message(&data, &mut session.seen_clients).await {
                            Ok(replies) => {
                                let mut failed = false;
                                for reply in replies {
                                    if sink.send(Message::Binary(Bytes::from(reply))).await.is_err() {
                                        failed = true;
                                        break;
                                    }
                                }
                                if failed {
                                    break;
                                }
                            }
                            Err(err) => {
                                tracing::debug!(page = page_id, "collab message error: {err}");
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if sink.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            outbound = room_rx.recv() => {
                match outbound {
                    Ok(frame) => {
                        if sink.send(Message::Binary(Bytes::from(frame))).await.is_err() {
                            break;
                        }
                    }
                    // Lagged too far behind the room broadcast: force a
                    // reconnect so the client resyncs from scratch.
                    Err(_) => break,
                }
            }
            _ = shutdown_rx.changed() => {
                let _ = sink.send(content_replaced_close()).await;
                await_close_ack(&mut stream).await;
                break;
            }
        }
    }

    state.collab.leave(session, Some(user.id)).await;
}
