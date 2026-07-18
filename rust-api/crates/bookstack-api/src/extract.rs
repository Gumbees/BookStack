use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use bookstack_core::models::{AuthUser, Role};
use bookstack_core::services::orgs;
use bookstack_core::{auth, Core, CoreError};

use crate::error::ApiError;
use crate::state::AppState;

/// Result of resolving credentials: the user plus an org hint carried by the
/// credential itself (API tokens and OAuth-issued JWTs are org-bound).
pub struct Resolved {
    pub user: AuthUser,
    pub org_hint: Option<i64>,
}

/// Resolve an `Authorization` header value (`Bearer <jwt>` or
/// BookStack-style `Token <id>:<secret>`).
pub async fn resolve_auth_header(core: &Core, value: &str) -> Result<Resolved, CoreError> {
    if let Some(jwt) = value.strip_prefix("Bearer ") {
        let (user, org_hint) = auth::verify_jwt_full(&core.db, &core.config.jwt_secret, jwt.trim()).await?;
        Ok(Resolved { user, org_hint })
    } else if let Some(token) = value.strip_prefix("Token ") {
        let (user, org_hint) = auth::verify_api_token(&core.db, token.trim()).await?;
        Ok(Resolved { user, org_hint })
    } else {
        Err(CoreError::Unauthorized)
    }
}

/// Resolve a bare credential string (used for WebSocket `?token=` params):
/// accepts a JWT or an API token `id:secret` pair.
pub async fn resolve_bare_token(core: &Core, value: &str) -> Result<Resolved, CoreError> {
    match auth::verify_jwt_full(&core.db, &core.config.jwt_secret, value).await {
        Ok((user, org_hint)) => Ok(Resolved { user, org_hint }),
        Err(_) if value.contains(':') => {
            let (user, org_hint) = auth::verify_api_token(&core.db, value).await?;
            Ok(Resolved { user, org_hint })
        }
        Err(err) => Err(err),
    }
}

/// Pick the active org for a request: explicit `X-Org-Id` header wins, then
/// the credential's own org binding, then the user's first membership.
pub async fn resolve_org(
    core: &Core,
    resolved: &Resolved,
    header_org: Option<i64>,
) -> Result<(i64, Role), CoreError> {
    let system_admin = resolved.user.role.is_admin();
    let org_id = match header_org.or(resolved.org_hint) {
        Some(org) => org,
        None => *orgs::accessible_org_ids(&core.db, resolved.user.id, system_admin)
            .await?
            .first()
            .ok_or(CoreError::Forbidden)?,
    };
    let role = orgs::role_in(&core.db, resolved.user.id, org_id, system_admin).await?;
    Ok((org_id, role))
}

fn header_org(parts: &Parts) -> Option<i64> {
    parts
        .headers
        .get("x-org-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok())
}

/// Extractor: the authenticated user (no org context). For system-level
/// endpoints (account, user admin, org management).
#[derive(Debug, Clone)]
pub struct AuthedUser(pub AuthUser);

impl AuthedUser {
    pub fn require_system_admin(&self) -> Result<(), ApiError> {
        if self.0.role.is_admin() {
            Ok(())
        } else {
            Err(ApiError(CoreError::Forbidden))
        }
    }
}

impl FromRequestParts<AppState> for AuthedUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ApiError(CoreError::Unauthorized))?;
        let resolved = resolve_auth_header(&state.core, header).await?;
        Ok(AuthedUser(resolved.user))
    }
}

/// Extractor: authenticated user + active org + their role inside it.
/// All content endpoints use this; every query is scoped to `org_id`.
#[derive(Debug, Clone)]
pub struct OrgCtx {
    pub user: AuthUser,
    pub org_id: i64,
    pub org_role: Role,
}

impl OrgCtx {
    pub fn require_edit(&self) -> Result<(), ApiError> {
        if self.org_role.can_edit() {
            Ok(())
        } else {
            Err(ApiError(CoreError::Forbidden))
        }
    }
}

impl FromRequestParts<AppState> for OrgCtx {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ApiError(CoreError::Unauthorized))?;
        let resolved = resolve_auth_header(&state.core, header).await?;
        let (org_id, org_role) = resolve_org(&state.core, &resolved, header_org(parts)).await?;
        Ok(OrgCtx { user: resolved.user, org_id, org_role })
    }
}
