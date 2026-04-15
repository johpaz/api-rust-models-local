use crate::gguf_parser::GGUFModelInfo;
use crate::metadata::{LayerInfo, ModelIndex};
use tracing::{debug, info};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

/// Identify which layer a tensor belongs to based on its name
fn extract_layer_index(tensor_name: &str) -> Option<usize> {
    // Pattern: "blk.N." where N is the layer index
    // Examples: "blk.0.attn_q.weight", "blk.12.ffn_down.weight"
    if let Some(start) = tensor_name.find("blk.") {
        let after_blk = &tensor_name[start + 4..];
        if let Some(dot_pos) = after_blk.find('.') {
            if let Ok(index) = after_blk[..dot_pos].parse::<usize>() {
                return Some(index);
            }
        }
    }
    None
}

/// Categorize tensor as layer-specific or global
enum TensorCategory {
    Layer(usize),
    TokenEmbedding,
    Output,
    Norm,
    Other,
}

fn categorize_tensor(tensor_name: &str, _arch: &str) -> TensorCategory {
    // Check if it's a layer tensor
    if let Some(layer_idx) = extract_layer_index(tensor_name) {
        return TensorCategory::Layer(layer_idx);
    }

    // Token embeddings
    if tensor_name.contains("token_embd") || tensor_name.contains("tok_embeddings") {
        return TensorCategory::TokenEmbedding;
    }

    // Output/lm_head
    if tensor_name.contains("output") || tensor_name.contains("lm_head") {
        return TensorCategory::Output;
    }

    // Normalization layers
    if tensor_name.contains("norm") {
        return TensorCategory::Norm;
    }

    TensorCategory::Other
}

/// Split a GGUF model into individual layer files
pub struct LayerSplitter {
    model_info: GGUFModelInfo,
    model_data: Vec<u8>,
    output_dir: PathBuf,
}

impl LayerSplitter {
    pub fn new<P: AsRef<Path>>(model_path: P, output_dir: P) -> Result<Self> {
        let model_path = model_path.as_ref();

        // Parse the model
        let model_info = crate::gguf_parser::parse_gguf(model_path)?;

        // Read entire file (we need this for tensor data extraction)
        let model_data = fs::read(model_path)
            .with_context(|| format!("Failed to read model file: {}", model_path.display()))?;

        let output_dir = output_dir.as_ref().to_path_buf();

        Ok(Self {
            model_info,
            model_data,
            output_dir,
        })
    }

    /// Extract raw bytes for a tensor
    fn extract_tensor_data(&self, tensor_name: &str, offset: u64, size: u64) -> Result<Vec<u8>> {
        // offset is relative to tensor_data_start
        let start = (self.model_info.tensor_data_start + offset) as usize;
        let end = start + size as usize;

        if end > self.model_data.len() {
            anyhow::bail!(
                "Tensor {} extends beyond file bounds: {} > {}",
                tensor_name,
                end,
                self.model_data.len()
            );
        }

        Ok(self.model_data[start..end].to_vec())
    }

    /// Run the full split process
    pub fn split(&self) -> Result<ModelIndex> {
        info!(
            "🔪 Splitting model into layers: {} tensors",
            self.model_info.tensors.len()
        );

        // Create output directory
        fs::create_dir_all(&self.output_dir)
            .with_context(|| format!("Failed to create output dir: {}", self.output_dir.display()))?;

        // Categorize tensors
        let mut layer_tensors: HashMap<usize, Vec<String>> = HashMap::new();
        let mut token_embd_tensors: Vec<String> = Vec::new();
        let mut output_tensors: Vec<String> = Vec::new();
        let mut norm_tensors: Vec<String> = Vec::new();

        let arch_str = match &self.model_info.architecture {
            crate::gguf_parser::ModelArch::Llama => "llama".to_string(),
            crate::gguf_parser::ModelArch::Mistral => "mistral".to_string(),
            crate::gguf_parser::ModelArch::Gemma => "gemma4".to_string(),
            crate::gguf_parser::ModelArch::Qwen => "qwen2".to_string(),
            crate::gguf_parser::ModelArch::Unknown(s) => s.clone(),
        };

        for tensor in &self.model_info.tensors {
            match categorize_tensor(&tensor.name, &arch_str) {
                TensorCategory::Layer(idx) => {
                    layer_tensors.entry(idx).or_default().push(tensor.name.clone());
                }
                TensorCategory::TokenEmbedding => {
                    token_embd_tensors.push(tensor.name.clone());
                }
                TensorCategory::Output => {
                    output_tensors.push(tensor.name.clone());
                }
                TensorCategory::Norm => {
                    norm_tensors.push(tensor.name.clone());
                }
                TensorCategory::Other => {
                    debug!("Skipping non-layer tensor: {}", tensor.name);
                }
            }
        }

        info!(
            "Categorized: {} layers, {} token_embd, {} output, {} norm",
            layer_tensors.len(),
            token_embd_tensors.len(),
            output_tensors.len(),
            norm_tensors.len()
        );

        // Create model index
        let n_layers = self.model_info.n_layers.unwrap_or(0) as usize;
        let mut index = ModelIndex::new(
            self.output_dir
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            arch_str,
            n_layers,
        );
        index.n_embd = self.model_info.n_embd;
        index.n_head = self.model_info.n_head;
        index.n_vocab = self.model_info.n_vocab;

        let mut total_size: u64 = 0;

        // Extract token embeddings
        if !token_embd_tensors.is_empty() {
            let file_name = "token_embd.bin".to_string();
            let file_path = self.output_dir.join(&file_name);
            let data = self.extract_layer_tensors(&token_embd_tensors)?;
            fs::write(&file_path, &data)?;
            index.token_embedding_tensors = token_embd_tensors.clone();
            total_size += data.len() as u64;
            info!(
                "✅ Token embeddings: {} ({:.2} MB)",
                file_path.display(),
                data.len() as f64 / (1024.0 * 1024.0)
            );
        }

        // Extract output layer
        if !output_tensors.is_empty() {
            let file_name = "output.bin".to_string();
            let file_path = self.output_dir.join(&file_name);
            let data = self.extract_layer_tensors(&output_tensors)?;
            fs::write(&file_path, &data)?;
            index.output_tensors = output_tensors.clone();
            total_size += data.len() as u64;
            info!(
                "✅ Output layer: {} ({:.2} MB)",
                file_path.display(),
                data.len() as f64 / (1024.0 * 1024.0)
            );
        }

        // Extract normalization layers
        if !norm_tensors.is_empty() {
            let file_name = "norm.bin".to_string();
            let file_path = self.output_dir.join(&file_name);
            let data = self.extract_layer_tensors(&norm_tensors)?;
            fs::write(&file_path, &data)?;
            index.norm_tensors = norm_tensors.clone();
            total_size += data.len() as u64;
            info!(
                "✅ Norm layers: {} ({:.2} MB)",
                file_path.display(),
                data.len() as f64 / (1024.0 * 1024.0)
            );
        }

        // Extract each transformer layer
        for layer_idx in 0..n_layers {
            let tensors = layer_tensors.get(&layer_idx);
            if tensors.is_none() || tensors.as_ref().unwrap().is_empty() {
                debug!("⚠️  No tensors found for layer {}", layer_idx);
                continue;
            }

            let file_name = format!("layer_{:03}.bin", layer_idx);
            let file_path = self.output_dir.join(&file_name);
            let tensor_names = tensors.cloned().unwrap_or_default();
            let data = self.extract_layer_tensors(&tensor_names)?;
            fs::write(&file_path, &data)?;

            let size = data.len() as u64;
            total_size += size;

            index.layers.push(LayerInfo {
                index: layer_idx,
                file_name: file_name.clone(),
                file_path: file_path.to_string_lossy().to_string(),
                tensor_names,
                size_bytes: size,
            });

            info!(
                "✅ Layer {:03}: {} ({:.2} MB)",
                layer_idx,
                file_path.display(),
                size as f64 / (1024.0 * 1024.0)
            );
        }

        index.total_size_bytes = total_size;

        // Save model index
        let index_path = self.output_dir.join("model_index.json");
        index.save(&index_path)?;
        info!(
            "📋 Model index saved: {} ({} files, {})",
            index_path.display(),
            index.file_count(),
            index.total_size_human()
        );

        Ok(index)
    }

    /// Extract multiple tensors and concatenate them with headers
    fn extract_layer_tensors(&self, tensor_names: &[String]) -> Result<Vec<u8>> {
        // Format: [num_tensors: u32][tensor1_name_len: u32][tensor1_name][tensor1_data_len: u64][tensor1_data]...
        let mut buffer = Vec::new();

        // Write number of tensors
        buffer.extend_from_slice(&(tensor_names.len() as u32).to_le_bytes());

        for name in tensor_names {
            // Find tensor info
            let tensor_info = self
                .model_info
                .tensors
                .iter()
                .find(|t| &t.name == name)
                .ok_or_else(|| anyhow::anyhow!("Tensor not found: {}", name))?;

            let data =
                self.extract_tensor_data(name, tensor_info.offset, tensor_info.size_bytes)?;

            // Write tensor header: name_len + name + data_len
            let name_bytes = name.as_bytes();
            buffer.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
            buffer.extend_from_slice(name_bytes);
            buffer.extend_from_slice(&(data.len() as u64).to_le_bytes());

            // Write tensor data
            buffer.extend_from_slice(&data);
        }

        Ok(buffer)
    }
}

/// Convenience function to split a GGUF model
pub fn split_gguf<P: AsRef<Path>>(model_path: P, output_dir: P) -> Result<ModelIndex> {
    let splitter = LayerSplitter::new(&model_path, &output_dir)?;
    splitter.split()
}
