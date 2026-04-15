//! Inference backend abstraction.
//!
//! Provides two backends:
//! - `LlamaServerBackend`: proxies requests to the external llama-server process
//! - `LayerStreamingBackend`: uses the built-in layer-streamer (AirLLM-style, low VRAM)

use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use layer_streamer::{
    parse_gguf, GGUFTokenizer, LayerLoader, ModelConfig, StreamingForward,
};

// ─────────────────────────────────────────────────────────────────────────────
// Request / Response types (OpenAI-compatible subset)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub model: String,
    pub prompt: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub model: String,
    pub text: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Layer-streaming backend
// ─────────────────────────────────────────────────────────────────────────────

/// Wraps `StreamingForward` + `GGUFTokenizer` for use as an HTTP inference backend.
///
/// The forward engine is protected by a `Mutex` because:
/// 1. It holds mutable KV-cache state.
/// 2. Inference is inherently sequential (one request at a time) — the GPU/CPU
///    is a single shared resource anyway.
pub struct LayerStreamingBackend {
    pub forward: Arc<Mutex<StreamingForward>>,
    pub tokenizer: Arc<GGUFTokenizer>,
    pub model_name: String,
}

impl LayerStreamingBackend {
    /// Load a GGUF model in layer-streaming mode.
    ///
    /// `model_path`: path to the `.gguf` file.
    /// `layers_dir`: directory containing the pre-split layer files
    ///               (produced by `layer-streamer split`).
    pub fn load(model_path: &str, layers_dir: &str) -> Result<Self> {
        let model_path = PathBuf::from(model_path);
        let layers_dir = PathBuf::from(layers_dir);

        tracing::info!(
            "LayerStreamingBackend: loading model {} from {}",
            model_path.display(),
            layers_dir.display()
        );

        let model_info = parse_gguf(&model_path)?;
        let config = ModelConfig::from_gguf(&model_info);
        let tokenizer = Arc::new(GGUFTokenizer::from_model_info(&model_info)?);

        let loader = LayerLoader::new(&layers_dir, &model_path)?;
        let forward = Arc::new(Mutex::new(StreamingForward::new(loader, config)?));

        let model_name = model_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        tracing::info!(
            "LayerStreamingBackend ready: model={}, vocab={}",
            model_name,
            tokenizer.vocab_size(),
        );

        Ok(Self {
            forward,
            tokenizer,
            model_name,
        })
    }

    /// Generate a completion for `request`.
    ///
    /// Runs synchronously in a `spawn_blocking` task so it does not block
    /// the Tokio async runtime.
    pub async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let forward = self.forward.clone();
        let tokenizer = self.tokenizer.clone();
        let model_name = self.model_name.clone();

        tokio::task::spawn_blocking(move || {
            // Tokenize prompt (add BOS)
            let prompt_tokens = tokenizer.encode(&request.prompt, true);
            let prompt_token_count = prompt_tokens.len();
            let eos_token = tokenizer.eos_token();

            tracing::info!(
                "complete: prompt_tokens={}, max_new_tokens={}, temp={}",
                prompt_token_count,
                request.max_tokens,
                request.temperature,
            );

            // Reset KV cache for a new independent request
            let mut fw = forward.lock().map_err(|e| anyhow::anyhow!("Mutex poisoned: {}", e))?;
            fw.reset();

            // Generate
            let new_tokens = fw.generate(
                &prompt_tokens,
                request.max_tokens,
                request.temperature,
                eos_token,
            )?;
            let completion_count = new_tokens.len();

            // Decode — skip BOS in the output
            let text = tokenizer.decode(&new_tokens);

            Ok(CompletionResponse {
                model: model_name,
                text,
                prompt_tokens: prompt_token_count,
                completion_tokens: completion_count,
            })
        })
        .await?
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Status
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime status of the layer-streaming backend.
#[derive(serde::Serialize)]
pub struct StreamingStatus {
    pub loaded: bool,
    pub model: Option<String>,
    pub vocab_size: Option<usize>,
    pub n_layers: Option<usize>,
}
