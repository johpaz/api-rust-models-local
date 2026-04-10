mod config;

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
use tokio::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::cors::CorsLayer;
use crate::config::Config;

#[derive(Clone)]
struct AppState {
    config: Config,
}

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

#[derive(Serialize)]
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
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let state = Arc::new(AppState { config: config.clone() });

    // Scan models at startup
    scan_models(&config.get_models_dir());

    let app = Router::new()
        .route("/", get(serve_ui))
        .route("/models.json", get(list_models))
        .route("/health", get(health_check))
        .route("/api/switch", post(switch_model))
        .route("/api/rescan", post(rescan_models))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    tracing::info!("🌐 LLM API Server listening on {}", addr);
    tracing::info!("🧠 llama-server: {}", config.llama_server_url);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

/// Serve the vision template HTML
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

/// Scan models directory and return list
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
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
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
    tracing::info!("📋 Found {} models", models.len());
    models
}

/// List available models
async fn list_models(state: axum::extract::State<Arc<AppState>>) -> Json<ModelsResponse> {
    let models = scan_models(&state.config.get_models_dir());
    let count = models.len();
    Json(ModelsResponse { models, count })
}

/// Rescan models
async fn rescan_models(state: axum::extract::State<Arc<AppState>>) -> Json<ModelsResponse> {
    let models = scan_models(&state.config.get_models_dir());
    let count = models.len();
    Json(ModelsResponse { models, count })
}

/// Health check - own + llama-server
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

    Json(HealthResponse {
        status: "ok".to_string(),
        llama_server: llama_status,
        active_model,
    })
}

/// Get currently active model from llama-server
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

/// Switch model - execute switch-model.sh script
async fn switch_model(
    state: axum::extract::State<Arc<AppState>>,
    Json(payload): Json<SwitchRequest>,
) -> Json<SwitchResponse> {
    let model = payload.model;
    let project_root = state.config.get_project_root();
    let models_dir = state.config.get_models_dir();

    // Verify model exists
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

    tracing::info!("🔄 Switching model to: {}", model);

    // Execute switch script in blocking thread (script takes ~1-2min)
    let script_path = switch_script.clone();
    let model_clone = model.clone();
    let result = tokio::task::spawn_blocking(move || {
        Command::new("bash")
            .arg(&script_path)
            .arg(&model_clone)
            .output()
    }).await;

    match result {
        Ok(Ok(out)) => {
            if out.status.success() {
                tracing::info!("✅ Model switched to: {}", model);
                Json(SwitchResponse {
                    status: "ok".to_string(),
                    model,
                    error: None,
                })
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                tracing::error!("❌ Switch failed: {}", stderr);
                Json(SwitchResponse {
                    status: "error".to_string(),
                    model,
                    error: Some(stderr.lines().last().unwrap_or(stdout.lines().last().unwrap_or("Unknown error")).to_string()),
                })
            }
        }
        Ok(Err(e)) => {
            tracing::error!("❌ Failed to execute switch script: {}", e);
            Json(SwitchResponse {
                status: "error".to_string(),
                model,
                error: Some(format!("Failed to execute: {}", e)),
            })
        }
        Err(e) => {
            tracing::error!("❌ Join error: {}", e);
            Json(SwitchResponse {
                status: "error".to_string(),
                model,
                error: Some(format!("Internal error: {}", e)),
            })
        }
    }
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
