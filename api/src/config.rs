use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub models_dir: String,
    pub inference_backend: String,
    pub layer_streaming_model: Option<String>,
    pub layer_streaming_layers_dir: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let current_dir = env::current_dir().expect("Failed to get current directory");
        let project_root = if current_dir.ends_with("api") {
            current_dir.parent().map(|p| p.to_path_buf()).unwrap_or_default()
        } else {
            current_dir
        };

        // Cargar .env desde la raíz del proyecto (funciona tanto si se ejecuta
        // desde api/ como desde la raíz)
        dotenvy::from_path(project_root.join(".env")).ok();

        let models_dir = env::var("MODELS_DIR")
            .unwrap_or_else(|_| project_root.join("models").to_string_lossy().to_string());

        Self {
            port: env::var("API_PORT")
                .or_else(|_| env::var("PORT"))
                .unwrap_or_else(|_| "3001".to_string())
                .parse()
                .expect("API_PORT debe ser un número"),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            models_dir,
            inference_backend: env::var("INFERENCE_BACKEND")
                .unwrap_or_else(|_| "layer_streaming".to_string()),
            layer_streaming_model: env::var("LAYER_STREAMING_MODEL").ok(),
            layer_streaming_layers_dir: env::var("LAYER_STREAMING_LAYERS_DIR").ok(),
        }
    }

    pub fn is_layer_streaming(&self) -> bool {
        self.inference_backend == "layer_streaming"
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
