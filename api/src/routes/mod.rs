pub mod chat;
pub mod health;
pub mod models;

use axum::{routing::{get, post}, Router};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::engine::InferenceActor;
use crate::middleware;

// ─── Estado compartido ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    /// Actor de inferencia. Metadatos legibles sin lock; ops via canal async.
    pub actor: Arc<RwLock<Option<InferenceActor>>>,
}

// ─── Router ───────────────────────────────────────────────────────────────────

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // UI & modelos
        .route("/", get(models::serve_ui))
        .route("/models.json", get(models::list_models))
        .route("/api/switch", post(models::switch_model))
        .route("/api/rescan", post(models::rescan_models))
        .route("/v1/models", get(models::oai_list_models))
        // Health & gestión del actor
        .route("/health", get(health::health_check))
        .route("/api/stream/load", post(health::stream_load))
        .route("/api/stream/status", get(health::stream_status))
        // Inferencia OpenAI-compatible
        .route("/v1/completions", post(chat::oai_completions))
        .route("/v1/chat/completions", post(chat::oai_chat_completions))
        .layer(middleware::cors())
        .with_state(state)
}
