mod config;
mod inference;

use axum::{
    routing::{get, post},
    Router,
    response::Html,
    Json,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::cors::CorsLayer;
use crate::config::Config;
use crate::inference::{CompletionRequest, LayerStreamingBackend, StreamingStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Shared state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    config: Config,
    /// Layer-streaming backend — loaded at startup (if configured) or on demand.
    streaming: Arc<RwLock<Option<LayerStreamingBackend>>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Request / response types (existing)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SwitchRequest {
    model: String,
}

#[derive(Serialize)]
struct SwitchResponse {
    status: String,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct ModelsResponse {
    models: Vec<ModelEntry>,
    count: usize,
}

#[derive(Serialize, Clone)]
struct ModelEntry {
    id: String,
    name: String,
    size_bytes: u64,
    size_human: String,
    path: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    llama_server: String,
    active_model: Option<String>,
    streaming_backend: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenAI-compatible request / response types
// ─────────────────────────────────────────────────────────────────────────────

/// POST /v1/completions
#[derive(Deserialize)]
struct OaiCompletionRequest {
    model: String,
    prompt: String,
    #[serde(default = "default_max_tokens")]
    max_tokens: usize,
    #[serde(default = "default_temperature")]
    temperature: f32,
}

fn default_max_tokens() -> usize { 256 }
fn default_temperature() -> f32 { 0.7 }

#[derive(Serialize)]
struct OaiCompletionResponse {
    id: String,
    object: String,
    model: String,
    choices: Vec<OaiChoice>,
    usage: OaiUsage,
}

#[derive(Serialize)]
struct OaiChoice {
    text: String,
    index: usize,
    finish_reason: String,
}

#[derive(Serialize)]
struct OaiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

/// POST /v1/chat/completions
#[derive(Deserialize)]
struct OaiChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(default = "default_max_tokens")]
    max_tokens: usize,
    #[serde(default = "default_temperature")]
    temperature: f32,
}

#[derive(Deserialize, Serialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OaiChatResponse {
    id: String,
    object: String,
    model: String,
    choices: Vec<ChatChoice>,
    usage: OaiUsage,
}

#[derive(Serialize)]
struct ChatChoice {
    message: ChatMessage,
    index: usize,
    finish_reason: String,
}

/// POST /api/stream/load
#[derive(Deserialize)]
struct StreamLoadRequest {
    model_path: String,
    layers_dir: String,
}

#[derive(Serialize)]
struct StreamLoadResponse {
    status: String,
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();

    // Scan models at startup
    scan_models(&config.get_models_dir());

    // Optionally initialise the layer-streaming backend
    let streaming: Arc<RwLock<Option<LayerStreamingBackend>>> =
        Arc::new(RwLock::new(None));

    if config.is_layer_streaming() {
        match (&config.layer_streaming_model, &config.layer_streaming_layers_dir) {
            (Some(model), Some(layers)) => {
                tracing::info!("Loading layer-streaming backend: {} -> {}", model, layers);
                match LayerStreamingBackend::load(model, layers) {
                    Ok(backend) => {
                        *streaming.write().await = Some(backend);
                        tracing::info!("Layer-streaming backend ready");
                    }
                    Err(e) => {
                        tracing::error!("Failed to load layer-streaming backend: {}", e);
                    }
                }
            }
            _ => {
                tracing::warn!(
                    "INFERENCE_BACKEND=layer_streaming but \
                     LAYER_STREAMING_MODEL / LAYER_STREAMING_LAYERS_DIR not set"
                );
            }
        }
    }

    let state = Arc::new(AppState {
        config: config.clone(),
        streaming,
    });

    let app = Router::new()
        // --- UI & management (existing) ---
        .route("/", get(serve_ui))
        .route("/models.json", get(list_models))
        .route("/health", get(health_check))
        .route("/api/switch", post(switch_model))
        .route("/api/rescan", post(rescan_models))
        // --- Layer-streaming management ---
        .route("/api/stream/load", post(stream_load))
        .route("/api/stream/status", get(stream_status))
        // --- OpenAI-compatible inference ---
        .route("/v1/models", get(oai_list_models))
        .route("/v1/completions", post(oai_completions))
        .route("/v1/chat/completions", post(oai_chat_completions))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    tracing::info!("LLM API Server listening on {}", addr);
    tracing::info!("llama-server: {}", config.llama_server_url);
    tracing::info!("Inference backend: {}", config.inference_backend);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// Existing handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn serve_ui(state: axum::extract::State<Arc<AppState>>) -> Html<String> {
    let project_root = state.config.get_project_root();
    let template_path = project_root.join("examples").join("vision-template.html");

    match fs::read_to_string(&template_path) {
        Ok(content) => Html(content),
        Err(e) => Html(format!(
            "<h1>UI not found</h1><p>Path: {}</p><p>{}</p>",
            template_path.display(),
            e
        )),
    }
}

fn scan_models(models_dir: &PathBuf) -> Vec<ModelEntry> {
    let mut models = Vec::new();

    if !models_dir.exists() {
        tracing::warn!("Models directory does not exist: {:?}", models_dir);
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
    tracing::info!("Found {} models", models.len());
    models
}

async fn list_models(state: axum::extract::State<Arc<AppState>>) -> Json<ModelsResponse> {
    let models = scan_models(&state.config.get_models_dir());
    let count = models.len();
    Json(ModelsResponse { models, count })
}

async fn rescan_models(state: axum::extract::State<Arc<AppState>>) -> Json<ModelsResponse> {
    let models = scan_models(&state.config.get_models_dir());
    let count = models.len();
    Json(ModelsResponse { models, count })
}

async fn health_check(state: axum::extract::State<Arc<AppState>>) -> Json<HealthResponse> {
    let client = reqwest::Client::new();
    let llama_status = match client
        .get(format!("{}/health", state.config.llama_server_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(_) => "ok".to_string(),
        Err(_) => "unreachable".to_string(),
    };

    let active_model = get_active_model(&state.config.llama_server_url).await;

    let streaming_status = {
        let guard = state.streaming.read().await;
        match &*guard {
            Some(b) => format!("loaded:{}", b.model_name),
            None => "not_loaded".to_string(),
        }
    };

    Json(HealthResponse {
        status: "ok".to_string(),
        llama_server: llama_status,
        active_model,
        streaming_backend: streaming_status,
    })
}

async fn get_active_model(llama_url: &str) -> Option<String> {
    let client = reqwest::Client::new();
    if let Ok(resp) = client
        .get(format!("{}/props", llama_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(path) = json.get("model_path").and_then(|v| v.as_str()) {
                    return path.split('/').last().map(|s| s.to_string());
                }
            }
        }
    }
    None
}

async fn switch_model(
    state: axum::extract::State<Arc<AppState>>,
    Json(payload): Json<SwitchRequest>,
) -> Json<SwitchResponse> {
    let model = payload.model;
    let project_root = state.config.get_project_root();
    let models_dir = state.config.get_models_dir();

    let model_path = models_dir.join(&model);
    if !model_path.exists() {
        return Json(SwitchResponse {
            status: "error".to_string(),
            model,
            error: Some(format!("Model file not found: {}", model_path.display())),
        });
    }

    let switch_script = project_root.join("scripts").join("switch-model.sh");
    if !switch_script.exists() {
        return Json(SwitchResponse {
            status: "error".to_string(),
            model,
            error: Some("switch-model.sh not found".to_string()),
        });
    }

    tracing::info!("Switching model to: {}", model);

    let script_path = switch_script.clone();
    let model_clone = model.clone();
    let result = tokio::task::spawn_blocking(move || {
        Command::new("bash")
            .arg(&script_path)
            .arg(&model_clone)
            .output()
    })
    .await;

    match result {
        Ok(Ok(out)) => {
            if out.status.success() {
                tracing::info!("Model switched to: {}", model);
                Json(SwitchResponse {
                    status: "ok".to_string(),
                    model,
                    error: None,
                })
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                tracing::error!("Switch failed: {}", stderr);
                Json(SwitchResponse {
                    status: "error".to_string(),
                    model,
                    error: Some(
                        stderr
                            .lines()
                            .last()
                            .unwrap_or(stdout.lines().last().unwrap_or("Unknown error"))
                            .to_string(),
                    ),
                })
            }
        }
        Ok(Err(e)) => Json(SwitchResponse {
            status: "error".to_string(),
            model,
            error: Some(format!("Failed to execute: {}", e)),
        }),
        Err(e) => Json(SwitchResponse {
            status: "error".to_string(),
            model,
            error: Some(format!("Internal error: {}", e)),
        }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Layer-streaming management handlers
// ─────────────────────────────────────────────────────────────────────────────

/// POST /api/stream/load
///
/// Load (or reload) the layer-streaming backend at runtime, without restarting
/// the API server.
///
/// ```json
/// { "model_path": "/models/gemma.gguf", "layers_dir": "/tmp/gemma-layers/" }
/// ```
async fn stream_load(
    state: axum::extract::State<Arc<AppState>>,
    Json(payload): Json<StreamLoadRequest>,
) -> Json<StreamLoadResponse> {
    tracing::info!(
        "stream/load: model={} layers={}",
        payload.model_path,
        payload.layers_dir,
    );

    let model_path = payload.model_path.clone();
    let layers_dir = payload.layers_dir.clone();

    let result: Result<anyhow::Result<LayerStreamingBackend>, tokio::task::JoinError> =
        tokio::task::spawn_blocking(move || {
            LayerStreamingBackend::load(&model_path, &layers_dir)
        })
        .await;

    match result {
        Ok(Ok(backend)) => {
            let model_name = backend.model_name.clone();
            *state.streaming.write().await = Some(backend);
            Json(StreamLoadResponse {
                status: "ok".to_string(),
                model: Some(model_name),
                error: None,
            })
        }
        Ok(Err(e)) => Json(StreamLoadResponse {
            status: "error".to_string(),
            model: None,
            error: Some(format!("{}", e)),
        }),
        Err(e) => Json(StreamLoadResponse {
            status: "error".to_string(),
            model: None,
            error: Some(format!("Internal error: {}", e)),
        }),
    }
}

/// GET /api/stream/status
async fn stream_status(
    state: axum::extract::State<Arc<AppState>>,
) -> Json<StreamingStatus> {
    let guard = state.streaming.read().await;
    match &*guard {
        Some(backend) => {
            let tokenizer = &backend.tokenizer;
            let fw = backend.forward.lock().unwrap();
            Json(StreamingStatus {
                loaded: true,
                model: Some(backend.model_name.clone()),
                vocab_size: Some(tokenizer.vocab_size()),
                n_layers: Some(fw.config().n_layers),
            })
        }
        None => Json(StreamingStatus {
            loaded: false,
            model: None,
            vocab_size: None,
            n_layers: None,
        }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenAI-compatible inference handlers
// ─────────────────────────────────────────────────────────────────────────────

/// GET /v1/models
async fn oai_list_models(
    state: axum::extract::State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let models = scan_models(&state.config.get_models_dir());
    let data: Vec<serde_json::Value> = models
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "object": "model",
                "owned_by": "local",
            })
        })
        .collect();
    Json(serde_json::json!({ "object": "list", "data": data }))
}

/// POST /v1/completions  (OpenAI-compatible)
///
/// Routes to the layer-streaming backend if loaded, otherwise returns an error
/// directing the client to use `/api/switch` + llama-server directly.
async fn oai_completions(
    state: axum::extract::State<Arc<AppState>>,
    Json(payload): Json<OaiCompletionRequest>,
) -> Json<serde_json::Value> {
    let guard = state.streaming.read().await;
    let backend = match &*guard {
        Some(b) => b,
        None => {
            return Json(serde_json::json!({
                "error": {
                    "message": "Layer-streaming backend not loaded. \
                                POST /api/stream/load first, or use llama-server on :8080.",
                    "type": "backend_unavailable",
                }
            }));
        }
    };

    let request = CompletionRequest {
        model: payload.model,
        prompt: payload.prompt,
        max_tokens: payload.max_tokens,
        temperature: payload.temperature,
    };

    match backend.complete(request).await {
        Ok(resp) => Json(serde_json::json!(OaiCompletionResponse {
            id: format!("cmpl-{}", uuid_simple()),
            object: "text_completion".to_string(),
            model: resp.model,
            choices: vec![OaiChoice {
                text: resp.text,
                index: 0,
                finish_reason: "stop".to_string(),
            }],
            usage: OaiUsage {
                prompt_tokens: resp.prompt_tokens,
                completion_tokens: resp.completion_tokens,
                total_tokens: resp.prompt_tokens + resp.completion_tokens,
            },
        })),
        Err(e) => Json(serde_json::json!({
            "error": { "message": format!("{}", e), "type": "inference_error" }
        })),
    }
}

/// POST /v1/chat/completions  (OpenAI-compatible)
///
/// Converts the messages array into a single prompt string and delegates to
/// the layer-streaming backend.
async fn oai_chat_completions(
    state: axum::extract::State<Arc<AppState>>,
    Json(payload): Json<OaiChatRequest>,
) -> Json<serde_json::Value> {
    // Build a simple chat prompt from the messages
    let prompt = payload
        .messages
        .iter()
        .map(|m| format!("<|{}|>\n{}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n<|assistant|>\n";

    let guard = state.streaming.read().await;
    let backend = match &*guard {
        Some(b) => b,
        None => {
            return Json(serde_json::json!({
                "error": {
                    "message": "Layer-streaming backend not loaded. \
                                POST /api/stream/load first.",
                    "type": "backend_unavailable",
                }
            }));
        }
    };

    let request = CompletionRequest {
        model: payload.model,
        prompt,
        max_tokens: payload.max_tokens,
        temperature: payload.temperature,
    };

    match backend.complete(request).await {
        Ok(resp) => Json(serde_json::json!(OaiChatResponse {
            id: format!("chatcmpl-{}", uuid_simple()),
            object: "chat.completion".to_string(),
            model: resp.model,
            choices: vec![ChatChoice {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: resp.text,
                },
                index: 0,
                finish_reason: "stop".to_string(),
            }],
            usage: OaiUsage {
                prompt_tokens: resp.prompt_tokens,
                completion_tokens: resp.completion_tokens,
                total_tokens: resp.prompt_tokens + resp.completion_tokens,
            },
        })),
        Err(e) => Json(serde_json::json!({
            "error": { "message": format!("{}", e), "type": "inference_error" }
        })),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal deterministic ID generator (no external dependency).
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", t)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Ctrl+C received, shutting down"),
        _ = terminate => tracing::info!("Terminate signal received, shutting down"),
    }
}
