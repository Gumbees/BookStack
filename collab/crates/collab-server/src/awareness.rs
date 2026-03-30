use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserState {
    pub user_id: i64,
    pub user_name: String,
    pub color: String,
    pub cursor_pos: u32,
    pub selection_start: u32,
    pub selection_end: u32,
}

impl UserState {
    pub fn new(user_id: i64, user_name: String) -> Self {
        Self {
            color: derive_color(user_id),
            user_id,
            user_name,
            cursor_pos: 0,
            selection_start: 0,
            selection_end: 0,
        }
    }
}

/// Deterministically derive a hex color from a user id.
fn derive_color(user_id: i64) -> String {
    // A small palette of distinct, readable colors.
    let colors = [
        "#e53935", "#8e24aa", "#1e88e5", "#00897b",
        "#43a047", "#fb8c00", "#6d4c41", "#546e7a",
        "#d81b60", "#039be5",
    ];
    let index = (user_id.unsigned_abs() as usize) % colors.len();
    colors[index].to_string()
}

/// Per-document awareness state: a map from client_id string to UserState.
#[derive(Default)]
pub struct AwarenessState {
    pub users: HashMap<String, UserState>,
}

impl AwarenessState {
    pub fn upsert(&mut self, client_id: String, state: UserState) {
        self.users.insert(client_id, state);
    }

    pub fn remove(&mut self, client_id: &str) {
        self.users.remove(client_id);
    }

    pub fn snapshot(&self) -> Vec<UserState> {
        self.users.values().cloned().collect()
    }
}
