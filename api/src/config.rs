use serde::Deserialize;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub port: u16,
    pub model_path: String,
    pub models_dir: String,
    pub api_token: String,
    pub context_size: u32,
    pub default_temperature: f32,
    pub max_concurrency: usize,
    pub host: String,
    pub rate_limit_requests: u64,
    pub rate_limit_seconds: u64,
    pub llama_server_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        // Get the project root directory (parent of the api/ directory)
        let current_dir = env::current_dir().expect("Failed to get current directory");
        let project_root = if current_dir.ends_with("api") {
            current_dir.parent().map(|p| p.to_path_buf()).unwrap_or_default()
        } else {
            current_dir
        };

        let models_dir = env::var("MODELS_DIR")
            .unwrap_or_else(|_| {
                let default_models = project_root.join("models");
                default_models.to_string_lossy().to_string()
            });

        Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a number"),
            model_path: env::var("MODEL_PATH")
                .unwrap_or_else(|_| {
                    let default_model = project_root.join("models").join("model.gguf");
                    default_model.to_string_lossy().to_string()
                }),
            models_dir,
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
            llama_server_url: env::var("LLAMA_SERVER_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        }
    }

    pub fn get_models_dir(&self) -> PathBuf {
        PathBuf::from(&self.models_dir)
    }
}
