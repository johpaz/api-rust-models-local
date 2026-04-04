use crate::config::Config;
use crate::error::AppError;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::error;

pub struct LlamaEngine {
    client: Client,
    server_url: String,
}

#[derive(Serialize)]
struct CompletionRequest {
    prompt: String,
    temperature: f32,
    n_predict: i32,
    stop: Vec<String>,
    stream: bool,
}

#[derive(Deserialize)]
struct CompletionChunk {
    content: String,
    stop: bool,
}

impl LlamaEngine {
    pub async fn new(config: Config) -> Result<Self, AppError> {
        let client = Client::new();
        let server_url = config.llama_server_url.clone();

        client
            .get(format!("{}/health", server_url))
            .send()
            .await
            .map_err(|e| AppError::ModelLoadError(format!(
                "Cannot reach llama-server at {}: {}",
                server_url, e
            )))?;

        tracing::info!("Connected to llama-server at {}", server_url);
        Ok(Self { client, server_url })
    }

    pub fn get_model_name(&self) -> String {
        "local-gguf-model".to_string()
    }

    pub async fn generate_stream(
        &self,
        prompt: String,
        temperature: f32,
        max_tokens: usize,
        stop: Vec<String>,
    ) -> Result<mpsc::Receiver<String>, AppError> {
        let (tx, rx) = mpsc::channel(100);

        let response = self
            .client
            .post(format!("{}/completion", self.server_url))
            .json(&CompletionRequest {
                prompt,
                temperature,
                n_predict: max_tokens as i32,
                stop,
                stream: true,
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
