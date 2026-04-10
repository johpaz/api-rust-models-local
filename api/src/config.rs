use serde::Deserialize;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub llama_server_url: String,
    pub models_dir: String,
    pub llama_server_path: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::from_path(env::current_dir().expect("Failed to get current directory").join(".env")).ok();

        let current_dir = env::current_dir().expect("Failed to get current directory");
        let project_root = if current_dir.ends_with("api") {
            current_dir.parent().map(|p| p.to_path_buf()).unwrap_or_default()
        } else {
            current_dir
        };

        let models_dir = env::var("MODELS_DIR")
            .unwrap_or_else(|_| project_root.join("models").to_string_lossy().to_string());

        let llama_server_path = env::var("LLAMA_SERVER_PATH")
            .unwrap_or_else(|_| project_root.join("llama-server/build-native/llama.cpp/build/bin/llama-server").to_string_lossy().to_string());

        Self {
            port: env::var("API_PORT")
                .or_else(|_| env::var("PORT"))
                .unwrap_or_else(|_| "3001".to_string())
                .parse()
                .expect("Port must be a number"),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            llama_server_url: env::var("LLAMA_SERVER_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            models_dir,
            llama_server_path,
        }
    }

    pub fn get_models_dir(&self) -> PathBuf {
        PathBuf::from(&self.models_dir)
    }

    pub fn get_project_root(&self) -> PathBuf {
        let current_dir = env::current_dir().expect("Failed to get current directory");
        if current_dir.ends_with("api") {
            current_dir.parent().map(|p| p.to_path_buf()).unwrap_or_default()
        } else {
            current_dir
        }
    }
}
