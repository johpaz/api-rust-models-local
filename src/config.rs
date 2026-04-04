use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub port: u16,
    pub api_token: String,
    pub default_temperature: f32,
    pub host: String,
    pub rate_limit_requests: u64,
    pub rate_limit_seconds: u64,
    pub llama_server_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a number"),
            api_token: env::var("API_TOKEN").expect("API_TOKEN must be set"),
            default_temperature: env::var("DEFAULT_TEMPERATURE")
                .unwrap_or_else(|_| "0.7".to_string())
                .parse()
                .expect("DEFAULT_TEMPERATURE must be a number"),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            rate_limit_requests: env::var("RATE_LIMIT_REQUESTS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .expect("RATE_LIMIT_REQUESTS must be a number"),
            rate_limit_seconds: env::var("RATE_LIMIT_SECONDS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .expect("RATE_LIMIT_SECONDS must be a number"),
            llama_server_url: env::var("LLAMA_SERVER_URL")
                .unwrap_or_else(|_| "http://llama-server:8080".to_string()),
        }
    }
}
