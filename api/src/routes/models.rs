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
}

pub async fn list_models(
    State(state): State<Arc<AppState>>,
) -> Json<ModelListResponse> {
    Json(ModelListResponse {
        object: "list".to_string(),
        data: vec![ModelData {
            id: state.engine.get_model_name(),
            object: "model".to_string(),
            created: 1712217600, // Fixed timestamp for now
            owned_by: "local".to_string(),
        }],
    })
}
