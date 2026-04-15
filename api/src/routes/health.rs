use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::engine::{InferenceActor, StreamingStatus};

use super::AppState;

// ─── Tipos ───────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub actor: String,
}

#[derive(Deserialize)]
pub struct StreamLoadRequest {
    pub model_path: String,
    pub layers_dir: String,
}

#[derive(Serialize)]
pub struct StreamLoadResponse {
    pub status: String,
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

pub async fn health_check(state: State<Arc<AppState>>) -> Json<HealthResponse> {
    let actor_status = {
        let g = state.actor.read().await;
        match &*g {
            Some(a) => format!("loaded:{}", a.model_name),
            None => "loading".to_string(),
        }
    };
    Json(HealthResponse {
        status: "ok".to_string(),
        actor: actor_status,
    })
}

/// POST /api/stream/load — carga o recarga el actor en caliente con rutas explícitas.
pub async fn stream_load(
    state: State<Arc<AppState>>,
    Json(payload): Json<StreamLoadRequest>,
) -> Json<StreamLoadResponse> {
    tracing::info!("stream/load: {} -> {}", payload.model_path, payload.layers_dir);
    match InferenceActor::load(&payload.model_path, &payload.layers_dir).await {
        Ok(actor) => {
            let name = actor.model_name.clone();
            *state.actor.write().await = Some(actor);
            Json(StreamLoadResponse {
                status: "ok".to_string(),
                model: Some(name),
                error: None,
            })
        }
        Err(e) => Json(StreamLoadResponse {
            status: "error".to_string(),
            model: None,
            error: Some(format!("{}", e)),
        }),
    }
}

/// GET /api/stream/status — sin locks, lee campos públicos del actor.
pub async fn stream_status(state: State<Arc<AppState>>) -> Json<StreamingStatus> {
    let g = state.actor.read().await;
    match &*g {
        Some(a) => Json(StreamingStatus {
            loaded: true,
            model: Some(a.model_name.clone()),
            vocab_size: Some(a.vocab_size),
            n_layers: Some(a.n_layers),
        }),
        None => Json(StreamingStatus {
            loaded: false,
            model: None,
            vocab_size: None,
            n_layers: None,
        }),
    }
}
