use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use bookstack_core::models::ListParams;
use bookstack_core::services::{books, chapters, pages, search, shelves, system, users};

use crate::error::ApiResult;
use crate::extract::AuthedUser;
use crate::state::AppState;

// ---- shelves ----

pub async fn list_shelves(
    State(state): State<AppState>,
    _user: AuthedUser,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(shelves::list(&state.core.db, &params).await?).unwrap()))
}

pub async fn get_shelf(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(shelves::get(&state.core.db, id).await?).unwrap()))
}

pub async fn get_shelf_by_slug(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(shelves::get_by_slug(&state.core.db, &slug).await?).unwrap()))
}

pub async fn create_shelf(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<shelves::CreateShelf>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    Ok(Json(serde_json::to_value(shelves::create(&state.core.db, user.0.id, &body).await?).unwrap()))
}

pub async fn update_shelf(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
    Json(body): Json<shelves::UpdateShelf>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    Ok(Json(serde_json::to_value(shelves::update(&state.core.db, user.0.id, id, &body).await?).unwrap()))
}

pub async fn delete_shelf(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    shelves::delete(&state.core.db, user.0.id, id).await?;
    Ok(Json(json!({ "deleted": true })))
}

// ---- books ----

pub async fn list_books(
    State(state): State<AppState>,
    _user: AuthedUser,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(books::list(&state.core.db, &params).await?).unwrap()))
}

pub async fn get_book(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(books::get(&state.core.db, id).await?).unwrap()))
}

pub async fn get_book_by_slug(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(books::get_by_slug(&state.core.db, &slug).await?).unwrap()))
}

pub async fn book_contents(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    books::fetch(&state.core.db, id).await?;
    Ok(Json(serde_json::to_value(books::contents(&state.core.db, id).await?).unwrap()))
}

pub async fn create_book(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<books::CreateBook>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    Ok(Json(serde_json::to_value(books::create(&state.core.db, user.0.id, &body).await?).unwrap()))
}

pub async fn update_book(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
    Json(body): Json<books::UpdateBook>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    Ok(Json(serde_json::to_value(books::update(&state.core.db, user.0.id, id, &body).await?).unwrap()))
}

pub async fn delete_book(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    books::delete(&state.core.db, user.0.id, id).await?;
    Ok(Json(json!({ "deleted": true })))
}

// ---- chapters ----

// NOTE: no #[serde(flatten)] here — axum's Query goes through
// serde_urlencoded, where flatten loses string→number coercion and breaks
// numeric params. Fields are spelled out and mapped to ListParams manually.
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
    _user: AuthedUser,
    Query(query): Query<ChapterListQuery>,
) -> ApiResult<Json<Value>> {
    let list = ListParams {
        count: query.count,
        offset: query.offset,
        sort: query.sort,
        order: query.order,
    };
    let result = chapters::list(&state.core.db, &list, query.book_id).await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

pub async fn get_chapter(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(chapters::get(&state.core.db, id).await?).unwrap()))
}

pub async fn create_chapter(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<chapters::CreateChapter>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    Ok(Json(serde_json::to_value(chapters::create(&state.core.db, user.0.id, &body).await?).unwrap()))
}

pub async fn update_chapter(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
    Json(body): Json<chapters::UpdateChapter>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    Ok(Json(serde_json::to_value(chapters::update(&state.core.db, user.0.id, id, &body).await?).unwrap()))
}

pub async fn delete_chapter(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    chapters::delete(&state.core.db, user.0.id, id).await?;
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
    _user: AuthedUser,
    Query(query): Query<PageListQuery>,
) -> ApiResult<Json<Value>> {
    let list = ListParams {
        count: query.count,
        offset: query.offset,
        sort: query.sort,
        order: query.order,
    };
    let result = pages::list(&state.core.db, &list, query.book_id, query.chapter_id).await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

pub async fn get_page(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(pages::get(&state.core.db, id).await?).unwrap()))
}

pub async fn get_page_by_slugs(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path((book_slug, page_slug)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let result = pages::get_by_slugs(&state.core.db, &book_slug, &page_slug).await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

pub async fn create_page(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<pages::CreatePage>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    Ok(Json(serde_json::to_value(pages::create(&state.core.db, user.0.id, &body).await?).unwrap()))
}

pub async fn update_page(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
    Json(body): Json<pages::UpdatePage>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    let (details, content_changed) = pages::update(&state.core.db, user.0.id, id, &body).await?;
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
    user: AuthedUser,
    Path(id): Path<i64>,
    Json(body): Json<MovePageRequest>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    let result = pages::move_page(&state.core.db, user.0.id, id, body.book_id, body.chapter_id).await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

pub async fn delete_page(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    pages::delete(&state.core.db, user.0.id, id).await?;
    state.collab.invalidate(id).await;
    Ok(Json(json!({ "deleted": true })))
}

pub async fn page_revisions(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(id): Path<i64>,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(pages::revisions(&state.core.db, id, &params).await?).unwrap()))
}

pub async fn restore_revision(
    State(state): State<AppState>,
    user: AuthedUser,
    Path((id, number)): Path<(i64, i32)>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    let result = pages::restore_revision(&state.core.db, user.0.id, id, number).await?;
    state.collab.invalidate(id).await;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

// ---- collaboration ----

pub async fn collab_save(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    user.require_edit()?;
    let flushed = state
        .collab
        .flush(id, Some(user.0.id))
        .await
        .map_err(|err| match err {
            bookstack_collab::CollabError::Core(core) => crate::error::ApiError(core),
            other => crate::error::ApiError(bookstack_core::CoreError::Internal(other.to_string())),
        })?;
    Ok(Json(json!({ "saved": flushed, "live_room": flushed })))
}

pub async fn collab_editors(
    State(state): State<AppState>,
    _user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
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
}

pub async fn search(
    State(state): State<AppState>,
    user: AuthedUser,
    Query(params): Query<SearchQuery>,
) -> ApiResult<Json<Value>> {
    let types: Vec<String> = params
        .types
        .as_deref()
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    let result = search::search(
        &state.core.db,
        &params.query,
        &types,
        params.count.unwrap_or(20),
        params.offset.unwrap_or(0),
        Some(user.0.id),
    )
    .await?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

// ---- users ----

pub async fn list_users(
    State(state): State<AppState>,
    user: AuthedUser,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<Value>> {
    user.require_admin()?;
    Ok(Json(serde_json::to_value(users::list(&state.core.db, &params).await?).unwrap()))
}

pub async fn get_user(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    if user.0.id != id {
        user.require_admin()?;
    }
    Ok(Json(serde_json::to_value(users::get(&state.core.db, id).await?).unwrap()))
}

pub async fn create_user(
    State(state): State<AppState>,
    user: AuthedUser,
    Json(body): Json<users::CreateUser>,
) -> ApiResult<Json<Value>> {
    user.require_admin()?;
    Ok(Json(serde_json::to_value(users::create(&state.core.db, &body).await?).unwrap()))
}

pub async fn update_user(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
    Json(body): Json<users::UpdateUser>,
) -> ApiResult<Json<Value>> {
    if user.0.id != id || body.role.is_some() {
        user.require_admin()?;
    }
    Ok(Json(serde_json::to_value(users::update(&state.core.db, id, &body).await?).unwrap()))
}

pub async fn delete_user(
    State(state): State<AppState>,
    user: AuthedUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    user.require_admin()?;
    users::delete(&state.core.db, id).await?;
    Ok(Json(json!({ "deleted": true })))
}

// ---- system ----

pub async fn system_info(State(state): State<AppState>, _user: AuthedUser) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(system::info(&state.core.db).await?).unwrap()))
}
