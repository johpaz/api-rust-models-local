mod config;
mod inference;

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::inference::{CompletionRequest, InferenceActor, StreamingStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Estado compartido
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    config: Config,
    /// Actor de inferencia. Metadatos legibles sin lock; ops via canal async.
    actor: Arc<RwLock<Option<InferenceActor>>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipos de request / response de gestión
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
// Tipos OpenAI-compatible
// ─────────────────────────────────────────────────────────────────────────────

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

#[derive(Deserialize)]
struct OaiChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(default = "default_max_tokens")]
    max_tokens: usize,
    #[serde(default = "default_temperature")]
    temperature: f32,
    #[serde(default)]
    stream: bool,
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
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    scan_models(&config.get_models_dir());

    let actor: Arc<RwLock<Option<InferenceActor>>> = Arc::new(RwLock::new(None));

    // Carga sin bloqueo: InferenceActor::load() usa spawn_blocking internamente.
    if config.is_layer_streaming() {
        match (&config.layer_streaming_model, &config.layer_streaming_layers_dir) {
            (Some(model), Some(layers)) => {
                tracing::info!("Cargando actor de inferencia: {} -> {}", model, layers);
                match InferenceActor::load(model, layers).await {
                    Ok(a) => {
                        tracing::info!("Actor listo: {}", a.model_name);
                        *actor.write().await = Some(a);
                    }
                    Err(e) => tracing::error!("No se pudo cargar el actor: {}", e),
                }
            }
            _ => tracing::warn!(
                "INFERENCE_BACKEND=layer_streaming pero faltan \
                 LAYER_STREAMING_MODEL / LAYER_STREAMING_LAYERS_DIR"
            ),
        }
    }

    let state = Arc::new(AppState { config: config.clone(), actor });

    let app = Router::new()
        // UI & gestión
        .route("/", get(serve_ui))
        .route("/models.json", get(list_models))
        .route("/health", get(health_check))
        .route("/api/switch", post(switch_model))
        .route("/api/rescan", post(rescan_models))
        // Gestión del actor
        .route("/api/stream/load", post(stream_load))
        .route("/api/stream/status", get(stream_status))
        // Inferencia OpenAI-compatible
        .route("/v1/models", get(oai_list_models))
        .route("/v1/completions", post(oai_completions))
        .route("/v1/chat/completions", post(oai_chat_completions))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    tracing::info!("LLM API Server en {}", addr);
    tracing::info!("Inference backend: {}", config.inference_backend);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers de gestión
// ─────────────────────────────────────────────────────────────────────────────

async fn serve_ui(state: State<Arc<AppState>>) -> Html<String> {
    let path = state.config.get_project_root().join("examples").join("vision-template.html");
    match fs::read_to_string(&path) {
        Ok(c) => Html(c),
        Err(e) => Html(format!("<h1>UI no encontrada</h1><p>{}</p><p>{}</p>", path.display(), e)),
    }
}

fn scan_models(models_dir: &PathBuf) -> Vec<ModelEntry> {
    let mut models = Vec::new();
    if !models_dir.exists() {
        tracing::warn!("Directorio de modelos no existe: {:?}", models_dir);
        return models;
    }
    if let Ok(entries) = fs::read_dir(models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "gguf") {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if name.starts_with("mmproj") { continue; }
                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let size_gb = size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                models.push(ModelEntry {
                    id: name.clone(), name: name.clone(), size_bytes,
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

async fn list_models(state: State<Arc<AppState>>) -> Json<ModelsResponse> {
    let models = scan_models(&state.config.get_models_dir());
    let count = models.len();
    Json(ModelsResponse { models, count })
}

async fn rescan_models(state: State<Arc<AppState>>) -> Json<ModelsResponse> {
    let models = scan_models(&state.config.get_models_dir());
    let count = models.len();
    Json(ModelsResponse { models, count })
}

async fn health_check(state: State<Arc<AppState>>) -> Json<HealthResponse> {
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
        let g = state.actor.read().await;
        match &*g {
            Some(a) => format!("loaded:{}", a.model_name),
            None => "not_loaded".to_string(),
        }
    };
    Json(HealthResponse { status: "ok".to_string(), llama_server: llama_status, active_model, streaming_backend: streaming_status })
}

async fn get_active_model(llama_url: &str) -> Option<String> {
    let client = reqwest::Client::new();
    if let Ok(resp) = client.get(format!("{}/props", llama_url)).timeout(Duration::from_secs(5)).send().await {
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

async fn switch_model(state: State<Arc<AppState>>, Json(payload): Json<SwitchRequest>) -> Json<SwitchResponse> {
    let model = payload.model;
    let root = state.config.get_project_root();
    let model_path = state.config.get_models_dir().join(&model);

    if !model_path.exists() {
        return Json(SwitchResponse { status: "error".to_string(), model, error: Some(format!("Modelo no encontrado: {}", model_path.display())) });
    }
    let switch_script = root.join("scripts").join("switch-model.sh");
    if !switch_script.exists() {
        return Json(SwitchResponse { status: "error".to_string(), model, error: Some("switch-model.sh no encontrado".to_string()) });
    }

    let script_path = switch_script.clone();
    let model_clone = model.clone();
    let result = tokio::task::spawn_blocking(move || Command::new("bash").arg(&script_path).arg(&model_clone).output()).await;

    match result {
        Ok(Ok(out)) if out.status.success() => Json(SwitchResponse { status: "ok".to_string(), model, error: None }),
        Ok(Ok(out)) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            Json(SwitchResponse {
                status: "error".to_string(), model,
                error: Some(stderr.lines().last().unwrap_or(stdout.lines().last().unwrap_or("Error")).to_string()),
            })
        }
        Ok(Err(e)) => Json(SwitchResponse { status: "error".to_string(), model, error: Some(format!("{}", e)) }),
        Err(e) => Json(SwitchResponse { status: "error".to_string(), model, error: Some(format!("Internal: {}", e)) }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Gestión del actor
// ─────────────────────────────────────────────────────────────────────────────

/// POST /api/stream/load — carga o recarga el actor en caliente.
async fn stream_load(state: State<Arc<AppState>>, Json(payload): Json<StreamLoadRequest>) -> Json<StreamLoadResponse> {
    tracing::info!("stream/load: {} -> {}", payload.model_path, payload.layers_dir);
    match InferenceActor::load(&payload.model_path, &payload.layers_dir).await {
        Ok(actor) => {
            let name = actor.model_name.clone();
            *state.actor.write().await = Some(actor);
            Json(StreamLoadResponse { status: "ok".to_string(), model: Some(name), error: None })
        }
        Err(e) => Json(StreamLoadResponse { status: "error".to_string(), model: None, error: Some(format!("{}", e)) }),
    }
}

/// GET /api/stream/status — sin ningún Mutex, lee campos públicos del actor.
async fn stream_status(state: State<Arc<AppState>>) -> Json<StreamingStatus> {
    let g = state.actor.read().await;
    match &*g {
        Some(a) => Json(StreamingStatus { loaded: true, model: Some(a.model_name.clone()), vocab_size: Some(a.vocab_size), n_layers: Some(a.n_layers) }),
        None => Json(StreamingStatus { loaded: false, model: None, vocab_size: None, n_layers: None }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inferencia OpenAI-compatible
// ─────────────────────────────────────────────────────────────────────────────

async fn oai_list_models(state: State<Arc<AppState>>) -> Json<serde_json::Value> {
    let models = scan_models(&state.config.get_models_dir());
    let data: Vec<serde_json::Value> = models.iter()
        .map(|m| serde_json::json!({ "id": m.id, "object": "model", "owned_by": "local" }))
        .collect();
    Json(serde_json::json!({ "object": "list", "data": data }))
}

async fn oai_completions(state: State<Arc<AppState>>, Json(payload): Json<OaiCompletionRequest>) -> Json<serde_json::Value> {
    let g = state.actor.read().await;
    let actor = match &*g {
        Some(a) => a,
        None => return Json(serde_json::json!({ "error": { "message": "Actor no cargado. POST /api/stream/load primero.", "type": "backend_unavailable" } })),
    };

    let req = CompletionRequest { model: payload.model, prompt: payload.prompt, max_tokens: payload.max_tokens, temperature: payload.temperature };

    match actor.complete(req).await {
        Ok(r) => Json(serde_json::json!(OaiCompletionResponse {
            id: format!("cmpl-{}", ts_id()), object: "text_completion".to_string(), model: r.model,
            choices: vec![OaiChoice { text: r.text, index: 0, finish_reason: "stop".to_string() }],
            usage: OaiUsage { prompt_tokens: r.prompt_tokens, completion_tokens: r.completion_tokens, total_tokens: r.prompt_tokens + r.completion_tokens },
        })),
        Err(e) => Json(serde_json::json!({ "error": { "message": format!("{}", e), "type": "inference_error" } })),
    }
}

/// POST /v1/chat/completions
///
/// `stream: false` (default) → JSON batch.
/// `stream: true`            → SSE token a token (compatible OpenAI).
async fn oai_chat_completions(state: State<Arc<AppState>>, Json(payload): Json<OaiChatRequest>) -> axum::response::Response {
    let prompt = payload.messages.iter()
        .map(|m| format!("<|{}|>\n{}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n") + "\n<|assistant|>\n";

    let req = CompletionRequest { model: payload.model.clone(), prompt, max_tokens: payload.max_tokens, temperature: payload.temperature };

    if payload.stream {
        chat_sse(state, req, payload.model).await
    } else {
        chat_batch(state, req, payload.model).await
    }
}

async fn chat_batch(state: State<Arc<AppState>>, req: CompletionRequest, model_id: String) -> axum::response::Response {
    let g = state.actor.read().await;
    let actor = match &*g {
        Some(a) => a,
        None => return Json(serde_json::json!({ "error": { "message": "Actor no cargado.", "type": "backend_unavailable" } })).into_response(),
    };
    match actor.complete(req).await {
        Ok(r) => Json(serde_json::json!(OaiChatResponse {
            id: format!("chatcmpl-{}", ts_id()), object: "chat.completion".to_string(), model: model_id,
            choices: vec![ChatChoice { message: ChatMessage { role: "assistant".to_string(), content: r.text }, index: 0, finish_reason: "stop".to_string() }],
            usage: OaiUsage { prompt_tokens: r.prompt_tokens, completion_tokens: r.completion_tokens, total_tokens: r.prompt_tokens + r.completion_tokens },
        })).into_response(),
        Err(e) => Json(serde_json::json!({ "error": { "message": format!("{}", e), "type": "inference_error" } })).into_response(),
    }
}

/// SSE: un evento por token. Señal de fin: `data: [DONE]`
async fn chat_sse(state: State<Arc<AppState>>, req: CompletionRequest, model_id: String) -> axum::response::Response {
    let g = state.actor.read().await;
    let actor = match &*g {
        Some(a) => a,
        None => return Json(serde_json::json!({ "error": { "message": "Actor no cargado.", "type": "backend_unavailable" } })).into_response(),
    };

    let rx = match actor.stream(req).await {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({ "error": { "message": format!("{}", e), "type": "inference_error" } })).into_response(),
    };

    drop(g); // liberar read lock antes de streamear

    let id = format!("chatcmpl-{}", ts_id());
    let mid = model_id.clone();

    let stream = UnboundedReceiverStream::new(rx).map(move |chunk| {
        let ev = match chunk {
            Some(text) => Event::default().data(
                serde_json::json!({
                    "id": id,
                    "object": "chat.completion.chunk",
                    "model": mid,
                    "choices": [{ "delta": { "content": text }, "index": 0, "finish_reason": null }]
                })
                .to_string(),
            ),
            None => Event::default().data("[DONE]"),
        };
        Ok::<Event, Infallible>(ev)
    });

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilidades
// ─────────────────────────────────────────────────────────────────────────────

fn ts_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:x}", t)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
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
        _ = ctrl_c => tracing::info!("Ctrl+C recibido, apagando"),
        _ = terminate => tracing::info!("SIGTERM recibido, apagando"),
    }
}
