use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use tracing::{debug, info};

use crate::gguf_parser::{GGMLType, GGUFModelInfo, parse_gguf};
use crate::metadata::ModelIndex;
use crate::tensor::Tensor;
use crate::dequantize::{self, dequantize};

/// A single layer's weights, fully dequantized to f32
#[derive(Clone)]
pub struct LayerWeights {
    pub index: usize,
    pub tensors: HashMap<String, Tensor>,
    pub size_bytes: u64,
}

/// Global (non-layer) weights
pub struct GlobalWeights {
    pub token_embd: HashMap<String, Tensor>,
    pub output: HashMap<String, Tensor>,
    pub norms: HashMap<String, Tensor>,
}

/// Loads tensors from GGUF using memory-mapped file (zero-copy, no RAM bloat)
pub struct LayerLoader {
    layers_dir: PathBuf,
    model_index: ModelIndex,
    /// Parsed GGUF metadata (tensors info, no data loaded)
    model_info: GGUFModelInfo,
    /// Memory-mapped GGUF file (only pages accessed on demand)
    gguf_mmap: memmap2::Mmap,
}

impl LayerLoader {
    pub fn new<P: AsRef<Path>>(layers_dir: P, ggu_path: P) -> Result<Self> {
        let layers_dir = layers_dir.as_ref().to_path_buf();
        let ggu_path = ggu_path.as_ref().to_path_buf();

        // Load model index
        let index_path = layers_dir.join("model_index.json");
        let model_index = ModelIndex::load(&index_path)
            .with_context(|| format!("Failed to load model index: {}", index_path.display()))?;

        // Parse GGUF ONCE (only reads metadata, not tensor data)
        let model_info = parse_gguf(&ggu_path)?;

        // Memory-map GGUF file (OS pages accessed on-demand, NOT fully loaded)
        let ggu_file = std::fs::File::open(&ggu_path)
            .with_context(|| format!("Failed to open GGUF file: {}", ggu_path.display()))?;

        // SAFETY: We don't modify the file, mmap is read-only
        // The OS will page in only the bytes we actually access
        let gguf_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&ggu_file)
                .with_context(|| format!("Failed to mmap GGUF file: {}", ggu_path.display()))?
        };

        info!("✅ LayerLoader initialized: {} layers, {} tensors, mmap size: {:.1} MB (zero-copy)",
            model_index.n_layers, model_info.tensors.len(),
            gguf_mmap.len() as f64 / (1024.0 * 1024.0));

        Ok(Self {
            layers_dir,
            model_index,
            model_info,
            gguf_mmap,
        })
    }

    pub fn n_layers(&self) -> usize {
        self.model_index.n_layers
    }

    pub fn n_embd(&self) -> Option<u32> {
        self.model_index.n_embd
    }

    /// Extract raw tensor bytes from mmap (zero-copy slice)
    fn extract_raw_tensor(&self, name: &str) -> Result<(&[u8], GGMLType, Vec<u64>)> {
        let tensor_info = self.model_info.tensors.iter()
            .find(|t| t.name == name)
            .ok_or_else(|| anyhow::anyhow!("Tensor not found: {}", name))?;

        let start = (self.model_info.tensor_data_start + tensor_info.offset) as usize;
        let end = start + tensor_info.size_bytes as usize;

        if end > self.gguf_mmap.len() {
            anyhow::bail!("Tensor {} extends beyond file: {} > {}", name, end, self.gguf_mmap.len());
        }

        Ok((
            &self.gguf_mmap[start..end],  // mmap slice - OS pages only
            tensor_info.dtype,
            tensor_info.dims.clone(),
        ))
    }

    /// Dequantize a tensor and load to RAM (this is the only place RAM is used)
    pub fn load_tensor(&self, name: &str) -> Result<Tensor> {
        let (raw_data, dtype, dims) = self.extract_raw_tensor(name)?;
        let n_elements: usize = dims.iter().map(|&d| d as usize).product();

        let f32_data = dequantize(raw_data, n_elements, dtype)?;
        let shape: Vec<usize> = dims.iter().map(|&d| d as usize).collect();
        Ok(Tensor::new(f32_data, shape))
    }

    /// Load a single layer by index
    pub fn load_layer(&self, layer_idx: usize) -> Result<LayerWeights> {
        if layer_idx >= self.model_index.n_layers {
            anyhow::bail!("Layer index {} out of range (max: {})", layer_idx, self.model_index.n_layers - 1);
        }

        let layer_info = &self.model_index.layers[layer_idx];

        debug!("Loading layer {} ({})", layer_idx, layer_info.file_name);

        let mut tensors = HashMap::new();
        for tensor_name in &layer_info.tensor_names {
            let tensor = self.load_tensor(tensor_name)?;
            tensors.insert(tensor_name.clone(), tensor);
        }

        Ok(LayerWeights {
            index: layer_idx,
            tensors,
            size_bytes: layer_info.size_bytes,
        })
    }

    /// Load global weights.
    ///
    /// Very large tensors (embedding tables, lm_head) are intentionally skipped
    /// here — access them lazily via `load_embedding_row()` / `compute_lm_head_logits()`.
    /// The threshold is 200 MB dequantized to f32.
    pub fn load_global(&self) -> Result<GlobalWeights> {
        const MAX_EAGER_BYTES: usize = 200 * 1024 * 1024; // 200 MB f32

        let mut token_embd = HashMap::new();
        let mut output = HashMap::new();
        let mut norms = HashMap::new();

        let is_large = |name: &str| -> bool {
            self.model_info.tensors.iter()
                .find(|t| t.name == name)
                .map(|ti| {
                    let n: usize = ti.dims.iter().map(|&d| d as usize).product();
                    n * 4 > MAX_EAGER_BYTES
                })
                .unwrap_or(false)
        };

        for name in &self.model_index.token_embedding_tensors {
            if is_large(name) {
                info!("⏭️  Skipping large tensor (lazy): {}", name);
                continue;
            }
            if let Ok(tensor) = self.load_tensor(name) {
                token_embd.insert(name.clone(), tensor);
            }
        }
        for name in &self.model_index.output_tensors {
            if is_large(name) {
                info!("⏭️  Skipping large tensor (lazy): {}", name);
                continue;
            }
            if let Ok(tensor) = self.load_tensor(name) {
                output.insert(name.clone(), tensor);
            }
        }
        for name in &self.model_index.norm_tensors {
            if is_large(name) {
                info!("⏭️  Skipping large tensor (lazy): {}", name);
                continue;
            }
            if let Ok(tensor) = self.load_tensor(name) {
                norms.insert(name.clone(), tensor);
            }
        }

        info!("✅ Global weights loaded: {} token_embd, {} output, {} norms",
            token_embd.len(), output.len(), norms.len());

        Ok(GlobalWeights { token_embd, output, norms })
    }

    /// Dequantize a single row of a 2D tensor from the mmap without loading the full tensor.
    ///
    /// GGUF stores tensors row-major with `dims[0]` as the innermost (row-width) dimension.
    /// This is the memory-efficient path for embedding lookups.
    pub fn load_tensor_row(&self, name: &str, row_idx: usize) -> Result<Vec<f32>> {
        let tensor_info = self.model_info.tensors.iter()
            .find(|t| t.name == name)
            .ok_or_else(|| anyhow::anyhow!("Tensor not found: {}", name))?;

        let n_cols = tensor_info.dims[0] as usize;
        let dtype = tensor_info.dtype;
        let tensor_start = (self.model_info.tensor_data_start + tensor_info.offset) as usize;

        let (row_start, row_bytes) = dequantize::row_byte_range(row_idx, n_cols, dtype);
        let abs_start = tensor_start + row_start;
        let abs_end = abs_start + row_bytes;

        if abs_end > self.gguf_mmap.len() {
            anyhow::bail!(
                "Row {} of {} out of bounds: bytes {}..{}, mmap len {}",
                row_idx, name, abs_start, abs_end, self.gguf_mmap.len()
            );
        }

        dequantize(&self.gguf_mmap[abs_start..abs_end], n_cols, dtype)
    }

    /// Compute full vocabulary logits: `logit[j] = dot(h, token_embd[j])`.
    ///
    /// Uses the mmap to dequantize one row at a time — no large heap allocation.
    /// For tied embeddings (Gemma, Llama), this is the lm_head projection.
    pub fn compute_lm_head_logits(&self, h: &[f32]) -> Result<Vec<f32>> {
        let tensor_name = self.model_index.token_embedding_tensors.iter()
            .find(|n| *n == "token_embd.weight")
            .or_else(|| self.model_index.token_embedding_tensors.first())
            .ok_or_else(|| anyhow::anyhow!("No token embedding tensor found"))?;

        let tensor_info = self.model_info.tensors.iter()
            .find(|t| t.name == tensor_name.as_str())
            .ok_or_else(|| anyhow::anyhow!("Tensor not found: {}", tensor_name))?;

        let n_cols = tensor_info.dims[0] as usize;   // n_embd
        let vocab_size = tensor_info.dims[1] as usize; // vocab tokens
        let dtype = tensor_info.dtype;
        let tensor_start = (self.model_info.tensor_data_start + tensor_info.offset) as usize;
        let bytes_per_row = dequantize::quantized_size(n_cols, dtype);

        if h.len() != n_cols {
            anyhow::bail!("lm_head: h.len()={} != n_embd={}", h.len(), n_cols);
        }

        let mut logits = vec![0.0f32; vocab_size];
        for j in 0..vocab_size {
            let start = tensor_start + j * bytes_per_row;
            let end = start + bytes_per_row;
            let row = dequantize(&self.gguf_mmap[start..end], n_cols, dtype)?;
            logits[j] = h.iter().zip(row.iter()).map(|(a, b)| a * b).sum();
        }
        Ok(logits)
    }

    pub fn architecture(&self) -> &str {
        &self.model_index.architecture
    }
}
