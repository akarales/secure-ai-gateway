//! API error taxonomy.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("detector unavailable: {0}")]
    Detector(String),
    #[error("model upstream error: {0}")]
    Model(String),
    #[error("bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Detector(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            ApiError::Model(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
