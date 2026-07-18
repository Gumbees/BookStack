mod error;
mod extract;
mod routes;
mod state;

use bookstack_core::{Config, Core};
use state::AppState;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=info")),
        )
        .init();

    let config = Config::from_env();
    let bind_addr = config.bind_addr.clone();
    tracing::info!("connecting to {}", config.database_url);
    let core = Core::connect(config).await?;
    let state = AppState::new(core);

    let app = routes::router(state.clone());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("bookstack-rs listening on http://{bind_addr} (REST: /api, collab: /ws, MCP: /mcp)");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("flushing collaborative documents before exit");
    state.collab.flush_all().await;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
