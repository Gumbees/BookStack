use std::sync::Arc;

use bookstack_collab::CollabEngine;
use bookstack_core::Core;
use bookstack_mcp::McpServer;
use bookstack_ops::WalShipper;
use bookstack_semantic::SemanticEngine;

#[derive(Clone)]
pub struct AppState {
    pub core: Core,
    pub collab: Arc<CollabEngine>,
    pub mcp: Arc<McpServer>,
    /// Outbound HTTP for SSO token/userinfo exchanges.
    pub http: reqwest::Client,
    /// Present when EMBEDDINGS_API_URL is configured.
    pub semantic: Option<Arc<SemanticEngine>>,
    /// Present when WALSHIP_ENABLED=true.
    pub walship: Option<Arc<WalShipper>>,
}

impl AppState {
    pub fn new(
        core: Core,
        semantic: Option<Arc<SemanticEngine>>,
        walship: Option<Arc<WalShipper>>,
    ) -> AppState {
        let collab = CollabEngine::new(core.clone());
        let mcp = Arc::new(McpServer::new(core.clone(), collab.clone(), semantic.clone()));
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("reqwest client");
        AppState { core, collab, mcp, http, semantic, walship }
    }
}
