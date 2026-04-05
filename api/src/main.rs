mod config;
mod error;
mod state;
mod middleware;
mod routes;
mod engine;

use axum::{
    routing::{get, post},
    Router,
    middleware::from_fn_with_state,
};
use axum::extract::DefaultBodyLimit;
use crate::config::Config;
use crate::state::AppState;
use crate::middleware::auth::auth_middleware;
use crate::routes::{chat, models, health, image, audio};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::{cors::CorsLayer, trace::TraceLayer, limit::RequestBodyLimitLayer};
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let state = Arc::new(AppState::new(config.clone()).await);

    // Rate limiting configuration
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(config.rate_limit_seconds)
            .burst_size(config.rate_limit_requests as u32)
            .finish()
            .unwrap(),
    );

    let app = Router::new()
        // Public routes
        .route("/health", get(health::health_check))
        // Protected routes (auth + rate limiting)
        .nest("/v1", Router::new()
            .route("/chat/completions", post(chat::chat_completions))
            .route("/models", get(models::list_models))
            .route("/images/generations", post(image::generate_image))
            .route("/audio/speech", post(audio::create_speech))
            .route("/audio/transcriptions", post(audio::create_transcription))
            .layer(from_fn_with_state(state.clone(), auth_middleware))
            .layer(GovernorLayer { config: governor_conf })
            .layer(DefaultBodyLimit::max(100 * 1024 * 1024)) // 100MB for audio/image uploads
        )
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    
    tracing::info!("Server listening on {}", addr);
    
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
        _ = ctrl_c => {
            tracing::info!("Ctrl+C received, shutting down");
        },
        _ = terminate => {
            tracing::info!("Terminate signal received, shutting down");
        },
    }
}
