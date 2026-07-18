pub mod admin;
pub mod auth;
pub mod branding;
pub mod content;
pub mod mcp;
pub mod oauth;
pub mod ops;
pub mod org;
pub mod sso;
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
        // orgs
        .route("/orgs", get(org::my_orgs).post(org::create_org))
        .route("/orgs/current", get(org::current))
        .route("/orgs/{org_id}/members", get(org::members).post(org::add_member))
        .route("/orgs/{org_id}/members/{member_id}", delete(org::remove_member))
        // branding (reads public; login page + header theming)
        .route("/branding", get(branding::effective))
        .route("/branding/logo", get(branding::logo))
        .route("/orgs/{org_id}/branding", get(branding::get_org_branding).put(branding::set_org_branding))
        .route("/orgs/{org_id}/branding/logo", post(branding::upload_org_logo).delete(branding::delete_org_logo))
        .route("/admin/branding", get(branding::get_global_branding).put(branding::set_global_branding))
        .route("/admin/branding/logo", post(branding::upload_global_logo).delete(branding::delete_global_logo))
        // SSO login (public: the login page needs these before auth)
        .route("/auth/providers", get(sso::public_providers))
        .route("/auth/oidc/{id}/start", get(sso::start))
        .route("/auth/oidc/callback", get(sso::callback))
        // admin settings: global + per-org auth config
        .route("/admin/settings", get(admin::get_global_settings).put(admin::update_global_settings))
        .route("/admin/auth-providers", get(admin::list_global_providers).post(admin::create_global_provider))
        .route("/admin/auth-providers/{id}", put(admin::update_global_provider).delete(admin::delete_global_provider))
        .route("/orgs/{org_id}/settings", get(admin::get_org_settings).put(admin::update_org_settings))
        .route("/orgs/{org_id}/auth-providers", get(admin::list_org_providers).post(admin::create_org_provider))
        .route("/orgs/{org_id}/auth-providers/{id}", put(admin::update_org_provider).delete(admin::delete_org_provider))
        // operational: imports, backups, WAL shipping
        .route("/admin/import", post(ops::start_import))
        .route("/admin/imports", get(ops::list_imports))
        .route("/admin/import/{id}", get(ops::get_import))
        .route("/admin/backups", get(ops::list_backups).post(ops::start_backup))
        .route("/admin/backups/{id}/verify", post(ops::verify_backup))
        .route("/admin/walship", get(ops::walship_status))
        // OAuth consent plumbing (authed SPA calls)
        .route("/oauth/client/{client_id}", get(oauth::client_info))
        .route("/oauth/approve", post(oauth::approve))
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
        .route("/search/semantic", get(content::semantic_search))
        .route("/search/reembed", post(content::reembed))
        .route("/search/embedding-status", get(content::embedding_status))
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
        // OAuth authorization server (MCP spec: RFC 8414 + 9728 discovery)
        .route("/.well-known/oauth-authorization-server", get(oauth::authorization_server_metadata))
        .route("/.well-known/oauth-authorization-server/mcp", get(oauth::authorization_server_metadata))
        .route("/.well-known/oauth-protected-resource", get(oauth::protected_resource_metadata))
        .route("/.well-known/oauth-protected-resource/mcp", get(oauth::protected_resource_metadata))
        .route("/oauth/register", post(oauth::register))
        .route("/oauth/authorize", get(oauth::authorize))
        .route("/oauth/token", post(oauth::token))
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
