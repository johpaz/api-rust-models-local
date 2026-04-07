use axum::{
    extract::State,
    Json,
};
use crate::state::AppState;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ModelListResponse {
    pub object: String,
    pub data: Vec<ModelData>,
}

#[derive(Debug, Serialize)]
pub struct ModelData {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
    pub name: String,
    pub path: String,
    pub size_bytes: Option<u64>,
}

pub async fn list_models(
    State(state): State<Arc<AppState>>,
) -> Json<ModelListResponse> {
    let models: Vec<ModelData> = state.available_models
        .iter()
        .map(|model| ModelData {
            id: model.id.clone(),
            object: "model".to_string(),
            created: 1712217600, // Fixed timestamp for now
            owned_by: "local".to_string(),
            name: model.name.clone(),
            path: model.path.clone(),
            size_bytes: model.size_bytes,
        })
        .collect();

    Json(ModelListResponse {
        object: "list".to_string(),
        data: models,
    })
}
