mod awareness;
mod sync;

use awareness::{LocalAwareness, parse_peers};
use sync::{
    MSG_AWARENESS, MSG_SYNC_STEP1, MSG_SYNC_STEP2, MSG_UPDATE,
    decode_prefixed, decode_state_vector, decode_update,
    encode_awareness_message, encode_sync_step1, encode_update_message,
};

use js_sys::Function;
use serde_wasm_bindgen::to_value as to_js;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{BinaryType, MessageEvent, WebSocket};
use yrs::{
    updates::encoder::Encode,
    Doc, GetString, ReadTxn, StateVector, Text, Transact, WriteTxn,
};

/// The main collaborative session exposed to JavaScript.
#[wasm_bindgen]
pub struct CollabSession {
    inner: Rc<RefCell<Inner>>,
}

struct Inner {
    doc: Doc,
    ws: WebSocket,
    local_awareness: LocalAwareness,
    on_update_cb: Option<Function>,
    on_awareness_cb: Option<Function>,
    connected: bool,
}

#[wasm_bindgen]
impl CollabSession {
    /// Create a new session. Connects to `ws_url` with the given JWT `token`.
    /// The `page_id` is used only to validate the document identity client-side.
    #[wasm_bindgen(constructor)]
    pub fn new(page_id: i64, ws_url: &str, token: &str) -> Result<CollabSession, JsValue> {
        let url = format!("{}/ws/page-{}?token={}", ws_url.trim_end_matches('/'), page_id, token);

        let ws = WebSocket::new(&url)?;
        ws.set_binary_type(BinaryType::Arraybuffer);

        let doc = Doc::new();

        let inner = Rc::new(RefCell::new(Inner {
            doc,
            ws: ws.clone(),
            local_awareness: LocalAwareness::default(),
            on_update_cb: None,
            on_awareness_cb: None,
            connected: false,
        }));

        // --- onopen ---
        {
            let inner_ref = Rc::clone(&inner);
            let onopen = Closure::once(Box::new(move |_: JsValue| {
                let mut guard = inner_ref.borrow_mut();
                guard.connected = true;
                // Send sync step 1: our current state vector.
                let sv = guard.doc.transact().state_vector();
                let msg = encode_sync_step1(&sv);
                let _ = guard.ws.send_with_u8_array(&msg);
            }) as Box<dyn FnOnce(JsValue)>);
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            onopen.forget();
        }

        // --- onmessage ---
        {
            let inner_ref = Rc::clone(&inner);
            let onmessage = Closure::wrap(Box::new(move |evt: MessageEvent| {
                let data = match evt.data().dyn_into::<js_sys::ArrayBuffer>() {
                    Ok(ab) => js_sys::Uint8Array::new(&ab).to_vec(),
                    Err(_) => return,
                };
                if data.is_empty() {
                    return;
                }
                let mut guard = inner_ref.borrow_mut();
                match data[0] {
                    MSG_SYNC_STEP1 => {
                        // Server sent its state vector; send back missing updates.
                        if let Some(sv_bytes) = decode_prefixed(&data[1..]) {
                            if let Some(sv) = decode_state_vector(&sv_bytes) {
                                let diff = guard.doc.transact().encode_diff_v1(&sv);
                                let resp = encode_update_message(&diff);
                                let _ = guard.ws.send_with_u8_array(&resp);
                            }
                        }
                    }
                    MSG_SYNC_STEP2 | MSG_UPDATE => {
                        if let Some(update_bytes) = decode_prefixed(&data[1..]) {
                            if let Some(update) = decode_update(&update_bytes) {
                                let _ = guard.doc.transact_mut().apply_update(update);
                                // Notify JS that the document changed.
                                if let Some(cb) = &guard.on_update_cb {
                                    let content = get_doc_text(&guard.doc);
                                    let _ = cb.call1(&JsValue::NULL, &JsValue::from_str(&content));
                                }
                            }
                        }
                    }
                    MSG_AWARENESS => {
                        if let Some(payload) = decode_prefixed(&data[1..]) {
                            let peers = parse_peers(&payload);
                            if let Some(cb) = &guard.on_awareness_cb {
                                if let Ok(js_peers) = to_js(&peers) {
                                    let _ = cb.call1(&JsValue::NULL, &js_peers);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }) as Box<dyn FnMut(MessageEvent)>);
            ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            onmessage.forget();
        }

        // --- onclose ---
        {
            let inner_ref = Rc::clone(&inner);
            let onclose = Closure::wrap(Box::new(move |_: JsValue| {
                inner_ref.borrow_mut().connected = false;
            }) as Box<dyn FnMut(JsValue)>);
            ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
            onclose.forget();
        }

        Ok(CollabSession { inner })
    }

    /// Get the current document text content.
    pub fn get_text(&self) -> String {
        let guard = self.inner.borrow();
        get_doc_text(&guard.doc)
    }

    /// Set the document text content (used by the HTML-based collab bridge).
    /// Replaces the "content" Text type in the Yrs doc and broadcasts the update.
    pub fn set_text(&self, text: &str) {
        let mut guard = self.inner.borrow_mut();
        {
            let mut txn = guard.doc.transact_mut();
            let content = txn.get_or_insert_text("content");
            let current_len = content.len(&txn);
            if current_len > 0 {
                content.remove_range(&mut txn, 0, current_len);
            }
            content.insert(&mut txn, 0, text);
        }
        // Encode and broadcast the resulting diff.
        let update_bytes = {
            let txn = guard.doc.transact();
            txn.encode_state_as_update_v1(&StateVector::default())
        };
        let msg = encode_update_message(&update_bytes);
        let _ = guard.ws.send_with_u8_array(&msg);
    }

    /// Apply a remote update (raw Yrs v1 update bytes) to the local document.
    pub fn apply_update(&self, update: &[u8]) {
        let mut guard = self.inner.borrow_mut();
        if let Some(u) = decode_update(update) {
            let _ = guard.doc.transact_mut().apply_update(u);
        }
    }

    /// Register a callback fired when the document changes due to a remote update.
    /// Callback receives the current document text as a string.
    pub fn on_update(&mut self, callback: Function) {
        self.inner.borrow_mut().on_update_cb = Some(callback);
    }

    /// Update local cursor/selection state and send it to the server.
    pub fn set_awareness(&self, cursor_pos: u32, selection_start: u32, selection_end: u32) {
        let mut guard = self.inner.borrow_mut();
        guard.local_awareness.update(cursor_pos, selection_start, selection_end);
        let payload = guard.local_awareness.to_json_bytes();
        let msg = encode_awareness_message(&payload);
        let _ = guard.ws.send_with_u8_array(&msg);
    }

    /// Register a callback fired when peer awareness state changes.
    /// Callback receives an array of peer state objects.
    pub fn on_awareness(&mut self, callback: Function) {
        self.inner.borrow_mut().on_awareness_cb = Some(callback);
    }

    /// Disconnect from the WebSocket.
    pub fn disconnect(&self) {
        let guard = self.inner.borrow();
        let _ = guard.ws.close();
    }

    /// Whether the WebSocket is currently connected.
    pub fn is_connected(&self) -> bool {
        self.inner.borrow().connected
    }
}

/// Extract the plain text from the Yrs document's default "content" text type.
fn get_doc_text(doc: &Doc) -> String {
    let txn = doc.transact();
    // We store content in a text type named "content".
    txn.get_text("content")
        .map(|t| t.get_string(&txn))
        .unwrap_or_default()
}
