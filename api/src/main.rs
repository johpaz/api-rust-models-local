mod config;
mod engine;
mod inference;
mod middleware;
mod routes;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::engine::InferenceActor;
use crate::routes::{AppState, build_router};

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
    routes::models::scan_models(&config.get_models_dir());

    let actor: Arc<RwLock<Option<InferenceActor>>> = Arc::new(RwLock::new(None));
    let state = Arc::new(AppState { config: config.clone(), actor });

    // Cargar modelo en background: el servidor acepta requests de inmediato.
    // Mientras el actor carga, /health y /v1/models responden; inferencia
    // devuelve "Actor no cargado" hasta que el load termina.
    if config.is_layer_streaming() {
        if let (Some(model), Some(layers)) = (
            config.layer_streaming_model.clone(),
            config.layer_streaming_layers_dir.clone(),
        ) {
            let actor_ref = state.actor.clone();
            tokio::spawn(async move {
                tracing::info!("Cargando actor en background: {}", model);
                match InferenceActor::load(&model, &layers).await {
                    Ok(a) => {
                        tracing::info!("Actor listo: {}", a.model_name);
                        *actor_ref.write().await = Some(a);
                    }
                    Err(e) => tracing::error!("No se pudo cargar el actor: {}", e),
                }
            });
        } else {
            tracing::warn!(
                "INFERENCE_BACKEND=layer_streaming pero faltan \
                 LAYER_STREAMING_MODEL / LAYER_STREAMING_LAYERS_DIR"
            );
        }
    }

    let app = build_router(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    tracing::info!("LLM API Server en {}", addr);
    tracing::info!("Inference backend: {}", config.inference_backend);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
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
        _ = ctrl_c => tracing::info!("Ctrl+C recibido, apagando"),
        _ = terminate => tracing::info!("SIGTERM recibido, apagando"),
    }
}
