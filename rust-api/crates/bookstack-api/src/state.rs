use std::sync::Arc;

use bookstack_collab::CollabEngine;
use bookstack_core::Core;
use bookstack_mcp::McpServer;

#[derive(Clone)]
pub struct AppState {
    pub core: Core,
    pub collab: Arc<CollabEngine>,
    pub mcp: Arc<McpServer>,
}

impl AppState {
    pub fn new(core: Core) -> AppState {
        let collab = CollabEngine::new(core.clone());
        let mcp = Arc::new(McpServer::new(core.clone(), collab.clone()));
        AppState { core, collab, mcp }
    }
}
