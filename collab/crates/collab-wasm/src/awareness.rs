use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerState {
    pub user_id: i64,
    pub user_name: String,
    pub color: String,
    pub cursor_pos: u32,
    pub selection_start: u32,
    pub selection_end: u32,
}

/// Local awareness state that we broadcast to the server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalAwareness {
    pub cursor_pos: u32,
    pub selection_start: u32,
    pub selection_end: u32,
}

impl LocalAwareness {
    pub fn update(&mut self, cursor_pos: u32, selection_start: u32, selection_end: u32) {
        self.cursor_pos = cursor_pos;
        self.selection_start = selection_start;
        self.selection_end = selection_end;
    }

    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// Parse a list of peer states from a server awareness broadcast.
pub fn parse_peers(payload: &[u8]) -> Vec<PeerState> {
    serde_json::from_slice::<Vec<PeerState>>(payload).unwrap_or_default()
}
