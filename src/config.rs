use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub port: u16,
    pub model_path: String,
    pub api_token: String,
    pub context_size: u32,
    pub default_temperature: f32,
    pub max_concurrency: usize,
    pub host: String,
    pub rate_limit_requests: u64,
    pub rate_limit_seconds: u64,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a number"),
            model_path: env::var("MODEL_PATH").expect("MODEL_PATH must be set"),
            api_token: env::var("API_TOKEN").expect("API_TOKEN must be set"),
            context_size: env::var("CONTEXT_SIZE")
                .unwrap_or_else(|_| "4096".to_string())
                .parse()
                .expect("CONTEXT_SIZE must be a number"),
            default_temperature: env::var("DEFAULT_TEMPERATURE")
                .unwrap_or_else(|_| "0.7".to_string())
                .parse()
                .expect("DEFAULT_TEMPERATURE must be a number"),
            max_concurrency: env::var("MAX_CONCURRENCY")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .expect("MAX_CONCURRENCY must be a number"),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            rate_limit_requests: env::var("RATE_LIMIT_REQUESTS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .expect("RATE_LIMIT_REQUESTS must be a number"),
            rate_limit_seconds: env::var("RATE_LIMIT_SECONDS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .expect("RATE_LIMIT_SECONDS must be a number"),
        }
    }
}
