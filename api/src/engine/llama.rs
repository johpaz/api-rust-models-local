use crate::config::Config;
use crate::error::AppError;
use crate::state::ModelInfo;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub struct LlamaEngine {
    client: Client,
    server_url: String,
    current_model: Mutex<String>,
    available_models: Vec<ModelInfo>,
}

#[derive(Serialize)]
struct CompletionRequest {
    prompt: String,
    temperature: f32,
    n_predict: i32,
    stop: Vec<String>,
    stream: bool,
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_data: Option<Vec<ImageData>>,
}

#[derive(Serialize)]
struct ImageData {
    data: String,
    id: i32,
}

#[derive(Deserialize)]
struct CompletionChunk {
    content: String,
    stop: bool,
}

#[derive(Deserialize, Debug)]
struct LlamaModelInfo {
    name: String,
    model: String,
}

#[derive(Deserialize, Debug)]
struct LlamaModelsResponse {
    models: Vec<LlamaModelInfo>,
}

impl LlamaEngine {
    pub async fn new(config: Config, available_models: Vec<ModelInfo>) -> Result<Self, AppError> {
        let client = Client::new();
        let server_url = config.llama_server_url.clone();

        // Check if llama-server is running
        client
            .get(format!("{}/health", server_url))
            .send()
            .await
            .map_err(|e| AppError::ModelLoadError(format!(
                "Cannot reach llama-server at {}: {}",
                server_url, e
            )))?;

        // Get the current model from llama-server
        let current_model = Self::fetch_current_model(&client, &server_url).await?;

        // If we have available models and the current model differs from first available, try switching
        if let Some(first_model) = available_models.first() {
            if current_model != first_model.name {
                info!("Current llama-server model ({}) differs from first available ({}), attempting switch", current_model, first_model.name);
                // Try to switch to first available model as default
                let _ = Self::try_switch_model(&client, &server_url, &first_model.name).await;
            }
        }

        tracing::info!("Connected to llama-server at {}", server_url);
        tracing::info!("Current model: {}", current_model);

        Ok(Self {
            client,
            server_url,
            current_model: Mutex::new(current_model),
            available_models,
        })
    }

    async fn fetch_current_model(client: &Client, server_url: &str) -> Result<String, AppError> {
        // Try the /v1/models endpoint first
        if let Ok(response) = client
            .get(format!("{}/v1/models", server_url))
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(models_resp) = response.json::<LlamaModelsResponse>().await {
                    if let Some(model) = models_resp.models.first() {
                        return Ok(model.name.clone());
                    }
                }
            }
        }

        // Fallback to /props endpoint
        if let Ok(response) = client
            .get(format!("{}/props", server_url))
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    if let Some(model_path) = json.get("model_path").and_then(|v| v.as_str()) {
                        // Extract just the filename from the path
                        if let Some(filename) = model_path.split('/').last() {
                            return Ok(filename.to_string());
                        }
                        return Ok(model_path.to_string());
                    }
                }
            }
        }

        // Default fallback
        Ok("unknown-model".to_string())
    }

    async fn try_switch_model(client: &Client, server_url: &str, model_name: &str) -> Result<(), String> {
        // Try POST /v1/models to load/switch model
        let load_response = client
            .post(format!("{}/v1/models", server_url))
            .json(&serde_json::json!({
                "model": model_name
            }))
            .send()
            .await;

        match load_response {
            Ok(resp) if resp.status().is_success() => {
                info!("Successfully switched to model: {}", model_name);
                Ok(())
            }
            Ok(resp) => {
                let status = resp.status();
                warn!("Model switching via POST /v1/models returned status: {}", status);
                Err(format!("Status: {}", status))
            }
            Err(e) => {
                warn!("Failed to switch model via API: {}", e);
                Err(e.to_string())
            }
        }
    }

    pub async fn get_model_name(&self) -> String {
        self.current_model.lock().await.clone()
    }

    pub async fn switch_model(&self, model_name: String) -> Result<(), AppError> {
        match Self::try_switch_model(&self.client, &self.server_url, &model_name).await {
            Ok(()) => {
                let mut model = self.current_model.lock().await;
                *model = model_name;
                Ok(())
            }
            Err(e) => {
                // If API doesn't support model switching, we still allow requests
                // The model parameter will be passed per-request to /completion
                warn!("Model switching not supported by llama-server, model param will be passed per-request: {}", e);
                let mut model = self.current_model.lock().await;
                *model = model_name;
                Ok(())
            }
        }
    }

    pub async fn generate_stream(
        &self,
        prompt: String,
        temperature: f32,
        max_tokens: usize,
        stop: Vec<String>,
        model: Option<String>,
    ) -> Result<mpsc::Receiver<String>, AppError> {
        let (tx, rx) = mpsc::channel(100);

        // Use the specified model, or fall back to current model
        let model_to_use = match model {
            Some(m) => m,
            None => self.get_model_name().await,
        };

        let response = self
            .client
            .post(format!("{}/completion", self.server_url))
            .json(&CompletionRequest {
                prompt,
                temperature,
                n_predict: max_tokens as i32,
                stop,
                stream: true,
                model: Some(model_to_use.clone()),
                image_data: None,
            })
            .send()
            .await
            .map_err(|e| AppError::EngineError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(AppError::EngineError(format!(
                "llama-server returned {}",
                response.status()
            )));
        }

        let mut byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();

            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();

                            if let Some(data) = line.strip_prefix("data: ") {
                                match serde_json::from_str::<CompletionChunk>(data) {
                                    Ok(c) if c.stop => return,
                                    Ok(c) if !c.content.is_empty() => {
                                        if tx.send(c.content).await.is_err() {
                                            return;
                                        }
                                    }
                                    Err(e) => error!("SSE parse error: {}", e),
                                    _ => {}
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Stream error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Generate text stream with image support
    pub async fn generate_stream_with_image(
        &self,
        prompt: String,
        temperature: f32,
        max_tokens: usize,
        stop: Vec<String>,
        model: Option<String>,
        image_base64: Option<String>,
    ) -> Result<mpsc::Receiver<String>, AppError> {
        let (tx, rx) = mpsc::channel(100);

        let model_to_use = match model {
            Some(m) => m,
            None => self.get_model_name().await,
        };

        // Prepare image data if provided
        let image_data = image_base64.map(|b64| vec![ImageData {
            data: b64,
            id: 1,
        }]);

        let response = self
            .client
            .post(format!("{}/completion", self.server_url))
            .json(&CompletionRequest {
                prompt,
                temperature,
                n_predict: max_tokens as i32,
                stop,
                stream: true,
                model: Some(model_to_use.clone()),
                image_data,
            })
            .send()
            .await
            .map_err(|e| AppError::EngineError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(AppError::EngineError(format!(
                "llama-server returned {}",
                response.status()
            )));
        }

        let mut byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();

            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();

                            if let Some(data) = line.strip_prefix("data: ") {
                                match serde_json::from_str::<CompletionChunk>(data) {
                                    Ok(c) if c.stop => return,
                                    Ok(c) if !c.content.is_empty() => {
                                        if tx.send(c.content).await.is_err() {
                                            return;
                                        }
                                    }
                                    Err(e) => error!("SSE parse error: {}", e),
                                    _ => {}
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Stream error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }
}
