use crate::config::Config;
use crate::engine::llama::LlamaEngine;
use std::sync::Arc;

pub struct AppState {
    pub config: Config,
    pub engine: Arc<LlamaEngine>,
}

impl AppState {
    pub async fn new(config: Config) -> Self {
        let engine = Arc::new(LlamaEngine::new(config.clone()).await.expect("Failed to connect to llama-server"));
        Self {
            config,
            engine,
        }
    }
}
