use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Inference engine error: {0}")]
    EngineError(String),

    #[error("Invalid token")]
    Unauthorized,

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Model load error: {0}")]
    ModelLoadError(String),

    #[error("Invalid request payload: {0}")]
    InvalidRequest(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Rate limit exceeded")]
    RateLimited,

    #[error("Model generation timeout")]
    Timeout,

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            AppError::ModelLoadError(_) | AppError::ConfigError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            AppError::EngineError(_) | AppError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            AppError::Timeout => (StatusCode::GATEWAY_TIMEOUT, self.to_string()),
            AppError::NotImplemented(_) => (StatusCode::NOT_IMPLEMENTED, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };

        let body = Json(json!({
            "error": {
                "message": message,
                "type": "invalid_request_error",
                "param": null,
                "code": status.as_u16(),
            }
        }));

        (status, body).into_response()
    }
}
