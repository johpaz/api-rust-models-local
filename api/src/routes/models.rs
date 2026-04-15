use axum::{extract::State, response::Html, Json};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::engine::InferenceActor;

use super::AppState;

// ─── Tipos ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SwitchRequest {
    pub model: String,
}

#[derive(Serialize)]
pub struct SwitchResponse {
    pub status: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelEntry>,
    pub count: usize,
}

#[derive(Serialize, Clone)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub size_human: String,
    pub path: String,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub fn scan_models(models_dir: &PathBuf) -> Vec<ModelEntry> {
    let mut models = Vec::new();
    if !models_dir.exists() {
        tracing::warn!("Directorio de modelos no existe: {:?}", models_dir);
        return models;
    }
    if let Ok(entries) = fs::read_dir(models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "gguf") {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if name.starts_with("mmproj") {
                    continue;
                }
                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let size_gb = size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                models.push(ModelEntry {
                    id: name.clone(),
                    name: name.clone(),
                    size_bytes,
                    size_human: format!("{:.1} GB", size_gb),
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
    }
    models.sort_by(|a, b| a.name.cmp(&b.name));
    tracing::info!("Modelos: {}", models.len());
    models
}

// ─── Handlers ────────────────────────────────────────────────────────────────

pub async fn serve_ui(state: State<Arc<AppState>>) -> Html<String> {
    let path = state
        .config
        .get_project_root()
        .join("examples")
        .join("vision-template.html");
    match fs::read_to_string(&path) {
        Ok(c) => Html(c),
        Err(e) => Html(format!(
            "<h1>UI no encontrada</h1><p>{}</p><p>{}</p>",
            path.display(),
            e
        )),
    }
}

pub async fn list_models(state: State<Arc<AppState>>) -> Json<ModelsResponse> {
    let models = scan_models(&state.config.get_models_dir());
    let count = models.len();
    Json(ModelsResponse { models, count })
}

pub async fn rescan_models(state: State<Arc<AppState>>) -> Json<ModelsResponse> {
    let models = scan_models(&state.config.get_models_dir());
    let count = models.len();
    Json(ModelsResponse { models, count })
}

/// POST /api/switch — cambia el modelo activo derivando el layers_dir
/// automáticamente por convención: models/layers/{nombre_sin_.gguf}/
pub async fn switch_model(
    state: State<Arc<AppState>>,
    Json(payload): Json<SwitchRequest>,
) -> Json<SwitchResponse> {
    let model = payload.model;
    let model_path = state.config.get_models_dir().join(&model);

    if !model_path.exists() {
        return Json(SwitchResponse {
            status: "error".to_string(),
            model,
            error: Some(format!("Modelo no encontrado: {}", model_path.display())),
        });
    }

    let model_stem = model.trim_end_matches(".gguf");
    let layers_dir = state
        .config
        .get_models_dir()
        .join("layers")
        .join(model_stem);

    let model_path_str = model_path.to_string_lossy().to_string();
    let layers_dir_str = layers_dir.to_string_lossy().to_string();

    tracing::info!("switch_model: {} -> {}", model_path_str, layers_dir_str);

    match InferenceActor::load(&model_path_str, &layers_dir_str).await {
        Ok(actor) => {
            let name = actor.model_name.clone();
            *state.actor.write().await = Some(actor);
            Json(SwitchResponse {
                status: "ok".to_string(),
                model: name,
                error: None,
            })
        }
        Err(e) => Json(SwitchResponse {
            status: "error".to_string(),
            model,
            error: Some(format!("{}", e)),
        }),
    }
}

/// GET /v1/models — lista compatible con OpenAI
pub async fn oai_list_models(state: State<Arc<AppState>>) -> Json<serde_json::Value> {
    let models = scan_models(&state.config.get_models_dir());
    let data: Vec<serde_json::Value> = models
        .iter()
        .map(|m| serde_json::json!({ "id": m.id, "object": "model", "owned_by": "local" }))
        .collect();
    Json(serde_json::json!({ "object": "list", "data": data }))
}
