mod auth;
mod awareness;
mod config;
mod document;
mod ws;

use axum::{
    extract::State,
    http::Method,
    response::Json,
    routing::get,
    Router,
};
use document::DocumentManager;
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    // Initialise structured tracing.
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Arc::new(config::Config::from_env());
    let doc_mgr = Arc::new(DocumentManager::new());

    info!(
        "Starting collab-server on port {}",
        config.port
    );

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any);

    let app = Router::new()
        .route("/ws/{document_id}", get(ws::ws_handler))
        .route("/health", get(health_handler))
        .route("/documents", get(documents_handler))
        .layer(cors)
        .with_state((Arc::clone(&config), Arc::clone(&doc_mgr)));

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");

    info!("Listening on {}", addr);
    axum::serve(listener, app).await.expect("server error");
}

async fn health_handler(
    State((_, doc_mgr)): State<(Arc<config::Config>, Arc<DocumentManager>)>,
) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "active_documents": doc_mgr.list_active().len(),
        "total_connections": doc_mgr.total_connections(),
    }))
}

async fn documents_handler(
    State((_, doc_mgr)): State<(Arc<config::Config>, Arc<DocumentManager>)>,
) -> Json<Value> {
    Json(json!({
        "documents": doc_mgr.list_active(),
    }))
}
