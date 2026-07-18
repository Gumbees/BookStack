pub mod auth;
pub mod content;
pub mod mcp;
pub mod ws;

use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        // auth
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::me))
        .route("/auth/tokens", get(auth::list_tokens).post(auth::create_token))
        .route("/auth/tokens/{id}", delete(auth::delete_token))
        // shelves
        .route("/shelves", get(content::list_shelves).post(content::create_shelf))
        .route("/shelves/{id}", get(content::get_shelf).put(content::update_shelf).delete(content::delete_shelf))
        .route("/shelves/slug/{slug}", get(content::get_shelf_by_slug))
        // books
        .route("/books", get(content::list_books).post(content::create_book))
        .route("/books/{id}", get(content::get_book).put(content::update_book).delete(content::delete_book))
        .route("/books/{id}/contents", get(content::book_contents))
        .route("/books/slug/{slug}", get(content::get_book_by_slug))
        // chapters
        .route("/chapters", get(content::list_chapters).post(content::create_chapter))
        .route("/chapters/{id}", get(content::get_chapter).put(content::update_chapter).delete(content::delete_chapter))
        // pages
        .route("/pages", get(content::list_pages).post(content::create_page))
        .route("/pages/{id}", get(content::get_page).put(content::update_page).delete(content::delete_page))
        .route("/pages/{id}/move", put(content::move_page))
        .route("/pages/{id}/revisions", get(content::page_revisions))
        .route("/pages/{id}/revisions/{number}/restore", post(content::restore_revision))
        .route("/pages/by-slugs/{book_slug}/{page_slug}", get(content::get_page_by_slugs))
        // collaboration controls
        .route("/pages/{id}/collab/save", post(content::collab_save))
        .route("/pages/{id}/collab/editors", get(content::collab_editors))
        // search / users / system
        .route("/search", get(content::search))
        .route("/users", get(content::list_users).post(content::create_user))
        .route("/users/{id}", get(content::get_user).put(content::update_user).delete(content::delete_user))
        .route("/system", get(content::system_info))
        .route("/healthz", get(|| async { "ok" }));

    let static_dir = state.core.config.static_dir.clone();
    let spa = ServeDir::new(&static_dir)
        .not_found_service(ServeFile::new(format!("{static_dir}/index.html")));

    Router::new()
        .nest("/api", api)
        .route("/ws/pages/{page_id}", get(ws::page_ws))
        .route("/mcp", post(mcp::handle_post).get(mcp::handle_get).delete(mcp::handle_delete))
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
