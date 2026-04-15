use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Metadata for a single layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerInfo {
    pub index: usize,
    pub file_name: String,
    pub file_path: String,
    pub tensor_names: Vec<String>,
    pub size_bytes: u64,
}

/// Model index - generated after splitting a GGUF into layers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelIndex {
    /// Original model file name
    pub source_model: String,
    /// Model architecture (llama, gemma, etc.)
    pub architecture: String,
    /// Number of transformer layers
    pub n_layers: usize,
    /// Embedding dimension
    pub n_embd: Option<u32>,
    /// Number of attention heads
    pub n_head: Option<u32>,
    /// Vocabulary size
    pub n_vocab: Option<u32>,
    /// Token embedding layer tensor names (GGUF tensor names)
    pub token_embedding_tensors: Vec<String>,
    /// Output layer (lm_head) tensor names
    pub output_tensors: Vec<String>,
    /// Normalization layer tensor names
    pub norm_tensors: Vec<String>,
    /// All transformer layers
    pub layers: Vec<LayerInfo>,
    /// Total size of all layer files
    pub total_size_bytes: u64,
    /// Generation timestamp
    pub created_at: u64,
}

impl ModelIndex {
    pub fn new(source_model: String, architecture: String, n_layers: usize) -> Self {
        Self {
            source_model,
            architecture,
            n_layers,
            n_embd: None,
            n_head: None,
            n_vocab: None,
            token_embedding_tensors: Vec::new(),
            output_tensors: Vec::new(),
            norm_tensors: Vec::new(),
            layers: Vec::with_capacity(n_layers),
            total_size_bytes: 0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Save index to JSON file
    pub fn save<P: Into<PathBuf>>(&self, path: P) -> std::io::Result<()> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load index from JSON file
    pub fn load<P: Into<PathBuf>>(path: P) -> std::io::Result<Self> {
        let path = path.into();
        let json = std::fs::read_to_string(&path)?;
        let index: ModelIndex = serde_json::from_str(&json)?;
        Ok(index)
    }

    /// Get total number of layer files
    pub fn file_count(&self) -> usize {
        self.layers.len()
            + (if !self.token_embedding_tensors.is_empty() { 1 } else { 0 })
            + (if !self.output_tensors.is_empty() { 1 } else { 0 })
            + (if !self.norm_tensors.is_empty() { 1 } else { 0 })
    }

    /// Get human-readable size
    pub fn total_size_human(&self) -> String {
        let bytes = self.total_size_bytes as f64;
        if bytes >= 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} GB", bytes / (1024.0 * 1024.0 * 1024.0))
        } else if bytes >= 1024.0 * 1024.0 {
            format!("{:.2} MB", bytes / (1024.0 * 1024.0))
        } else {
            format!("{:.2} KB", bytes / 1024.0)
        }
    }
}
