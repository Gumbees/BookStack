use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use bookstack_core::models::AuthUser;
use bookstack_core::{auth, Core, CoreError};

use crate::error::ApiError;
use crate::state::AppState;

/// Resolve an `Authorization` header value (`Bearer <jwt>` or
/// BookStack-style `Token <id>:<secret>`).
pub async fn resolve_auth_header(core: &Core, value: &str) -> Result<AuthUser, CoreError> {
    if let Some(jwt) = value.strip_prefix("Bearer ") {
        auth::verify_jwt(&core.db, &core.config.jwt_secret, jwt.trim()).await
    } else if let Some(token) = value.strip_prefix("Token ") {
        auth::verify_api_token(&core.db, token.trim()).await
    } else {
        Err(CoreError::Unauthorized)
    }
}

/// Resolve a bare credential string (used for WebSocket `?token=` params):
/// accepts a JWT or an API token `id:secret` pair.
pub async fn resolve_bare_token(core: &Core, value: &str) -> Result<AuthUser, CoreError> {
    match auth::verify_jwt(&core.db, &core.config.jwt_secret, value).await {
        Ok(user) => Ok(user),
        Err(_) if value.contains(':') => auth::verify_api_token(&core.db, value).await,
        Err(err) => Err(err),
    }
}

/// Extractor: the authenticated user for this request.
#[derive(Debug, Clone)]
pub struct AuthedUser(pub AuthUser);

impl AuthedUser {
    pub fn require_edit(&self) -> Result<(), ApiError> {
        if self.0.role.can_edit() {
            Ok(())
        } else {
            Err(ApiError(CoreError::Forbidden))
        }
    }

    pub fn require_admin(&self) -> Result<(), ApiError> {
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
        let user = resolve_auth_header(&state.core, header).await?;
        Ok(AuthedUser(user))
    }
}
