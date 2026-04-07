use crate::config::Config;
use crate::engine::llama::LlamaEngine;
use serde::Serialize;
use std::fs;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: Option<u64>,
}

pub struct AppState {
    pub config: Config,
    pub engine: Arc<LlamaEngine>,
    pub available_models: Vec<ModelInfo>,
}

impl AppState {
    pub async fn new(config: Config) -> Self {
        let engine = Arc::new(LlamaEngine::new(config.clone()).await.expect("Failed to initialize llama engine"));
        
        // Scan models directory for available models
        let available_models = Self::scan_models_directory(&config);
        
        Self {
            config,
            engine,
            available_models,
        }
    }

    fn scan_models_directory(config: &Config) -> Vec<ModelInfo> {
        let models_dir = config.get_models_dir();
        let mut models = Vec::new();

        if !models_dir.exists() {
            tracing::warn!("Models directory does not exist: {:?}", models_dir);
            return models;
        }

        // Read directory and filter for .gguf files
        let entries = match fs::read_dir(&models_dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::error!("Failed to read models directory: {}", e);
                return models;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            
            // Check if file has .gguf extension
            if path.extension().map_or(false, |ext| ext == "gguf") {
                let file_name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                
                let size_bytes = entry.metadata().map(|m| m.len()).ok();
                
                models.push(ModelInfo {
                    id: file_name.clone(),
                    name: file_name.clone(),
                    path: path.to_string_lossy().to_string(),
                    size_bytes,
                });
            }
        }

        // Sort models by name
        models.sort_by(|a, b| a.name.cmp(&b.name));
        
        tracing::info!("Found {} models in {:?}", models.len(), models_dir);
        models
    }

    pub fn find_model_by_name(&self, model_name: &str) -> Option<ModelInfo> {
        self.available_models
            .iter()
            .find(|m| m.id == model_name || m.name == model_name)
            .cloned()
    }
}
