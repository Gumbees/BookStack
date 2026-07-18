use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use bookstack_core::CoreError;

/// HTTP wrapper for core errors: maps domain failures to status codes and a
/// stable JSON error envelope.
#[derive(Debug)]
pub struct ApiError(pub CoreError);

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        ApiError(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            CoreError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            CoreError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            CoreError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            CoreError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            other => {
                tracing::error!("internal error: {other}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".to_string())
            }
        };
        (status, Json(json!({ "error": { "code": status.as_u16(), "message": message } }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
