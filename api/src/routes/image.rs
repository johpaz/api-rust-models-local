use axum::{
    extract::State,
    Json,
};
use crate::state::AppState;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Deserialize)]
pub struct ImageGenerationRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default = "default_image_size")]
    pub size: String,
    #[serde(default)]
    pub n: usize,
    #[serde(default)]
    pub response_format: String,
}

fn default_image_size() -> String {
    "1024x1024".to_string()
}

#[derive(Debug, Serialize)]
pub struct ImageGenerationResponse {
    pub created: i64,
    pub data: Vec<ImageObject>,
}

#[derive(Debug, Serialize)]
pub struct ImageObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b64_json: Option<String>,
}

pub async fn generate_image(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<ImageGenerationRequest>,
) -> Result<Json<ImageGenerationResponse>, AppError> {
    // For now, this is a placeholder.
    // llama-server doesn't natively support image generation via GGUF.
    // This would require a separate image generation model (FLUX, SD).
    // The endpoint exists for API compatibility with OpenAI format.

    tracing::info!("Image generation request: {}", payload.prompt);

    Err(AppError::NotImplemented(
        "Image generation requires a dedicated image generation model (FLUX, SD). llama.cpp does not support this natively.".to_string()
    ))
}
