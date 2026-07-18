use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use bookstack_core::models::ListParams;
use bookstack_core::services::{books, chapters, orgs, pages, search, shelves, system, users};

use crate::error::ApiResult;
use crate::extract::{AuthedUser, OrgCtx};
use crate::state::AppState;

// ---- shelves ----

pub async fn list_shelves(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(shelves::list(&state.core.db, ctx.org_id, &params).await?).unwrap()))
}

pub async fn get_shelf(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(shelves::get(&state.core.db, ctx.org_id, id).await?).unwrap()))
}

pub async fn get_shelf_by_slug(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(slug): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(shelves::get_by_slug(&state.core.db, ctx.org_id, &slug).await?).unwrap()))
}

pub async fn create_shelf(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Json(body): Json<shelves::CreateShelf>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    Ok(Json(
        serde_json::to_value(shelves::create(&state.core.db, ctx.org_id, ctx.user.id, &body).await?).unwrap(),
    ))
}

pub async fn update_shelf(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
    Json(body): Json<shelves::UpdateShelf>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    Ok(Json(
        serde_json::to_value(shelves::update(&state.core.db, ctx.org_id, ctx.user.id, id, &body).await?)
            .unwrap(),
    ))
}

pub async fn delete_shelf(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    shelves::delete(&state.core.db, ctx.org_id, ctx.user.id, id).await?;
    Ok(Json(json!({ "deleted": true })))
}

// ---- books ----

pub async fn list_books(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(books::list(&state.core.db, ctx.org_id, &params).await?).unwrap()))
}

pub async fn get_book(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(books::get(&state.core.db, ctx.org_id, id).await?).unwrap()))
}

pub async fn get_book_by_slug(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(slug): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(books::get_by_slug(&state.core.db, ctx.org_id, &slug).await?).unwrap()))
}

pub async fn book_contents(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(books::contents(&state.core.db, ctx.org_id, id).await?).unwrap()))
}

pub async fn create_book(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Json(body): Json<books::CreateBook>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    Ok(Json(
        serde_json::to_value(books::create(&state.core.db, ctx.org_id, ctx.user.id, &body).await?).unwrap(),
    ))
}

pub async fn update_book(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
    Json(body): Json<books::UpdateBook>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    Ok(Json(
        serde_json::to_value(books::update(&state.core.db, ctx.org_id, ctx.user.id, id, &body).await?)
            .unwrap(),
    ))
}

pub async fn delete_book(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    books::delete(&state.core.db, ctx.org_id, ctx.user.id, id).await?;
    Ok(Json(json!({ "deleted": true })))
}

// ---- chapters ----

#[derive(Deserialize)]
pub struct ChapterListQuery {
    pub book_id: Option<i64>,
    pub count: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

pub async fn list_chapters(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Query(query): Query<ChapterListQuery>,
) -> ApiResult<Json<Value>> {
    let list = ListParams {
        count: query.count,
        offset: query.offset,
        sort: query.sort,
        order: query.order,
    };
    let result = chapters::list(&state.core.db, ctx.org_id, &list, query.book_id).await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

pub async fn get_chapter(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(chapters::get(&state.core.db, ctx.org_id, id).await?).unwrap()))
}

pub async fn create_chapter(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Json(body): Json<chapters::CreateChapter>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    Ok(Json(
        serde_json::to_value(chapters::create(&state.core.db, ctx.org_id, ctx.user.id, &body).await?)
            .unwrap(),
    ))
}

pub async fn update_chapter(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
    Json(body): Json<chapters::UpdateChapter>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    Ok(Json(
        serde_json::to_value(chapters::update(&state.core.db, ctx.org_id, ctx.user.id, id, &body).await?)
            .unwrap(),
    ))
}

pub async fn delete_chapter(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    chapters::delete(&state.core.db, ctx.org_id, ctx.user.id, id).await?;
    Ok(Json(json!({ "deleted": true })))
}

// ---- pages ----

#[derive(Deserialize)]
pub struct PageListQuery {
    pub book_id: Option<i64>,
    pub chapter_id: Option<i64>,
    pub count: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

pub async fn list_pages(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Query(query): Query<PageListQuery>,
) -> ApiResult<Json<Value>> {
    let list = ListParams {
        count: query.count,
        offset: query.offset,
        sort: query.sort,
        order: query.order,
    };
    let result = pages::list(&state.core.db, ctx.org_id, &list, query.book_id, query.chapter_id).await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

pub async fn get_page(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(pages::get(&state.core.db, ctx.org_id, id).await?).unwrap()))
}

pub async fn get_page_by_slugs(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path((book_slug, page_slug)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let result = pages::get_by_slugs(&state.core.db, ctx.org_id, &book_slug, &page_slug).await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

pub async fn create_page(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Json(body): Json<pages::CreatePage>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    Ok(Json(
        serde_json::to_value(pages::create(&state.core.db, ctx.org_id, ctx.user.id, &body).await?).unwrap(),
    ))
}

pub async fn update_page(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
    Json(body): Json<pages::UpdatePage>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    let (details, content_changed) =
        pages::update(&state.core.db, ctx.org_id, ctx.user.id, id, &body).await?;
    if content_changed {
        // Content replaced out-of-band: drop any live CRDT room so editors
        // reconnect against the new content.
        state.collab.invalidate(id).await;
    }
    Ok(Json(serde_json::to_value(details).unwrap()))
}

#[derive(Deserialize)]
pub struct MovePageRequest {
    pub book_id: i64,
    pub chapter_id: Option<i64>,
}

pub async fn move_page(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
    Json(body): Json<MovePageRequest>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    let result =
        pages::move_page(&state.core.db, ctx.org_id, ctx.user.id, id, body.book_id, body.chapter_id).await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

pub async fn delete_page(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    pages::delete(&state.core.db, ctx.org_id, ctx.user.id, id).await?;
    state.collab.invalidate(id).await;
    Ok(Json(json!({ "deleted": true })))
}

pub async fn page_revisions(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        serde_json::to_value(pages::revisions(&state.core.db, ctx.org_id, id, &params).await?).unwrap(),
    ))
}

pub async fn restore_revision(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path((id, number)): Path<(i64, i32)>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    let result = pages::restore_revision(&state.core.db, ctx.org_id, ctx.user.id, id, number).await?;
    state.collab.invalidate(id).await;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

// ---- collaboration ----

pub async fn collab_save(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    // Page must exist within the caller's org.
    pages::fetch(&state.core.db, ctx.org_id, id).await?;
    let flushed = state
        .collab
        .flush(id, Some(ctx.user.id))
        .await
        .map_err(|err| match err {
            bookstack_collab::CollabError::Core(core) => crate::error::ApiError(core),
            other => crate::error::ApiError(bookstack_core::CoreError::Internal(other.to_string())),
        })?;
    Ok(Json(json!({ "saved": flushed, "live_room": flushed })))
}

pub async fn collab_editors(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    pages::fetch(&state.core.db, ctx.org_id, id).await?;
    Ok(Json(json!({ "editors": state.collab.active_editors(id) })))
}

// ---- search ----

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub query: String,
    pub types: Option<String>,
    pub count: Option<i64>,
    pub offset: Option<i64>,
    /// "org" (default) searches the active org; "global" searches every org
    /// the user belongs to.
    pub scope: Option<String>,
}

pub async fn search(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Query(params): Query<SearchQuery>,
) -> ApiResult<Json<Value>> {
    let types: Vec<String> = params
        .types
        .as_deref()
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    let org_ids: Vec<i64> = if params.scope.as_deref() == Some("global") {
        orgs::accessible_org_ids(&state.core.db, ctx.user.id, ctx.user.role.is_admin()).await?
    } else {
        vec![ctx.org_id]
    };
    let result = search::search(
        &state.core.db,
        &org_ids,
        &params.query,
        &types,
        params.count.unwrap_or(20),
        params.offset.unwrap_or(0),
        Some(ctx.user.id),
    )
    .await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

// ---- semantic search ----

#[derive(Deserialize)]
pub struct SemanticQuery {
    #[serde(default)]
    pub query: String,
    pub mode: Option<String>,
    pub count: Option<i64>,
    pub scope: Option<String>,
}

pub async fn semantic_search(
    State(state): State<AppState>,
    ctx: OrgCtx,
    Query(params): Query<SemanticQuery>,
) -> ApiResult<Json<Value>> {
    let Some(engine) = &state.semantic else {
        return Err(crate::error::ApiError(bookstack_core::CoreError::validation(
            "semantic search is not configured on this instance (set EMBEDDINGS_API_URL)",
        )));
    };
    let org_ids: Vec<i64> = if params.scope.as_deref() == Some("global") {
        orgs::accessible_org_ids(&state.core.db, ctx.user.id, ctx.user.role.is_admin()).await?
    } else {
        vec![ctx.org_id]
    };
    let mode = bookstack_semantic::Mode::parse(params.mode.as_deref().unwrap_or("precision"));
    let response = engine
        .search(&org_ids, &params.query, mode, params.count.unwrap_or(20).clamp(1, 50) as usize)
        .await
        .map_err(|e| crate::error::ApiError(bookstack_core::CoreError::Internal(e.to_string())))?;
    Ok(Json(serde_json::to_value(response).unwrap()))
}

pub async fn reembed(State(state): State<AppState>, ctx: OrgCtx) -> ApiResult<Json<Value>> {
    ctx.require_edit()?;
    let Some(engine) = &state.semantic else {
        return Err(crate::error::ApiError(bookstack_core::CoreError::validation(
            "semantic search is not configured on this instance",
        )));
    };
    let queued = engine
        .reembed_org(ctx.org_id)
        .await
        .map_err(|e| crate::error::ApiError(bookstack_core::CoreError::Internal(e.to_string())))?;
    Ok(Json(json!({ "queued": queued })))
}

pub async fn embedding_status(State(state): State<AppState>, ctx: OrgCtx) -> ApiResult<Json<Value>> {
    match &state.semantic {
        Some(engine) => {
            let status = engine
                .status(ctx.org_id)
                .await
                .map_err(|e| crate::error::ApiError(bookstack_core::CoreError::Internal(e.to_string())))?;
            Ok(Json(serde_json::to_value(status).unwrap()))
        }
        None => Ok(Json(json!({ "enabled": false }))),
    }
}

// ---- users (system admin) ----

pub async fn list_users(
    State(state): State<AppState>,
    user: AuthedUser,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    Ok(Json(serde_json::to_value(users::list(&state.core.db, &params).await?).unwrap()))
}

pub async fn get_user(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    if user.0.id != id {
        user.require_system_admin()?;
    }
    Ok(Json(serde_json::to_value(users::get(&state.core.db, id).await?).unwrap()))
}

pub async fn create_user(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<users::CreateUser>,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    Ok(Json(serde_json::to_value(users::create(&state.core.db, &body).await?).unwrap()))
}

pub async fn update_user(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
    Json(body): Json<users::UpdateUser>,
) -> ApiResult<Json<Value>> {
    if user.0.id != id || body.role.is_some() {
        user.require_system_admin()?;
    }
    Ok(Json(serde_json::to_value(users::update(&state.core.db, id, &body).await?).unwrap()))
}

pub async fn delete_user(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    user.require_system_admin()?;
    users::delete(&state.core.db, id).await?;
    Ok(Json(json!({ "deleted": true })))
}

// ---- system ----

pub async fn system_info(State(state): State<AppState>, ctx: OrgCtx) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(system::info(&state.core.db, ctx.org_id).await?).unwrap()))
}
