use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

use crate::gguf_parser::{GGUFModelInfo, GGUFValue, ModelArch};
use crate::tensor::Tensor;
use crate::layer_loader::{LayerLoader, LayerWeights, GlobalWeights};
use crate::rope::Rope;
use crate::sampler::{sample, SamplingStrategy};

/// KV Cache for a single layer: stores K and V tensors across all generated tokens
pub struct LayerKvCache {
    pub k_cache: Tensor, // [n_kv_heads * seq_len * head_dim]
    pub v_cache: Tensor, // [n_kv_heads * seq_len * head_dim]
    pub seq_len: usize,
}

/// Complete inference state: KV cache across all layers
pub struct InferenceState {
    pub kv_cache: Vec<LayerKvCache>,
    pub seq_len: usize,
    pub max_seq_len: usize,
}

impl InferenceState {
    pub fn new(n_layers: usize, n_kv_heads: usize, head_dim: usize, max_seq_len: usize) -> Self {
        let kv_cache: Vec<LayerKvCache> = (0..n_layers)
            .map(|_| LayerKvCache {
                k_cache: Tensor::zeros(&[max_seq_len * n_kv_heads * head_dim]),
                v_cache: Tensor::zeros(&[max_seq_len * n_kv_heads * head_dim]),
                seq_len: 0,
            })
            .collect();

        Self { kv_cache, seq_len: 0, max_seq_len }
    }

    pub fn seq_len(&self) -> usize {
        self.seq_len
    }
}

/// Generic model configuration, auto-detected from GGUF metadata.
///
/// Supports Llama, Gemma, Mistral, Qwen and any other transformer-based
/// architecture stored in GGUF format.
#[derive(Clone)]
pub struct ModelConfig {
    /// Architecture string as found in GGUF ("llama", "gemma", "mistral", "qwen2", …)
    pub architecture: String,
    pub n_layers: usize,
    pub n_embd: usize,
    pub n_head: usize,
    pub n_kv_head: usize,
    pub head_dim: usize,
    pub n_intermediate: usize,
    pub vocab_size: usize,
    pub rope_theta: f32,
}

/// Backward-compatible alias kept for existing CLI code.
pub type GemmaConfig = ModelConfig;

impl ModelConfig {
    /// Build a `ModelConfig` from parsed GGUF metadata.
    ///
    /// All fields are read from the model's key-value section.
    /// Falls back to sensible defaults when a key is missing.
    pub fn from_gguf(info: &GGUFModelInfo) -> Self {
        // Use the raw arch string from GGUF (e.g. "gemma4", "qwen3") so that
        // key lookups like "gemma4.embedding_length" work correctly.
        // Falling back through the enum would silently drop the version suffix.
        let arch_str = info.key_values.get("general.architecture")
            .and_then(|v| if let GGUFValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_else(|| match &info.architecture {
                ModelArch::Llama => "llama".to_string(),
                ModelArch::Mistral => "mistral".to_string(),
                ModelArch::Gemma => "gemma".to_string(),
                ModelArch::Qwen => "qwen2".to_string(),
                ModelArch::Unknown(s) => s.clone(),
            });

        let get_u32 = |key: &str| -> Option<u32> {
            info.key_values.get(key).and_then(|v| match v {
                GGUFValue::U32(n) => Some(*n),
                GGUFValue::U64(n) => Some(*n as u32),
                GGUFValue::I32(n) => Some(*n as u32),
                _ => None,
            })
        };
        let get_f32 = |key: &str| -> Option<f32> {
            info.key_values.get(key).and_then(|v| match v {
                GGUFValue::F32(n) => Some(*n),
                GGUFValue::F64(n) => Some(*n as f32),
                _ => None,
            })
        };

        let n_layers =
            get_u32(&format!("{}.block_count", arch_str)).unwrap_or(32) as usize;
        let n_embd =
            get_u32(&format!("{}.embedding_length", arch_str)).unwrap_or(4096) as usize;
        let n_head =
            get_u32(&format!("{}.attention.head_count", arch_str)).unwrap_or(32) as usize;
        let n_kv_head =
            get_u32(&format!("{}.attention.head_count_kv", arch_str))
                .unwrap_or(n_head as u32) as usize;
        let n_intermediate =
            get_u32(&format!("{}.feed_forward_length", arch_str)).unwrap_or(11008) as usize;
        let rope_theta =
            get_f32(&format!("{}.rope.freq_base", arch_str)).unwrap_or(10000.0);

        // Vocab size: prefer the tokenizer array length, fall back to general field
        let vocab_size = {
            let from_tokens = info
                .key_values
                .get("tokenizer.ggml.tokens")
                .and_then(|v| {
                    if let GGUFValue::Array(arr) = v {
                        Some(arr.len() as u32)
                    } else {
                        None
                    }
                });
            from_tokens
                .or(info.n_vocab)
                .or_else(|| get_u32("general.vocab_size"))
                .unwrap_or(32000) as usize
        };

        let head_dim = if n_head > 0 { n_embd / n_head } else { 128 };

        info!("ModelConfig from GGUF: arch={}, layers={}, embd={}, heads={}/{}, ff={}, vocab={}, rope_theta={}",
            arch_str, n_layers, n_embd, n_head, n_kv_head, n_intermediate, vocab_size, rope_theta);

        Self {
            architecture: arch_str,
            n_layers,
            n_embd,
            n_head,
            n_kv_head,
            head_dim,
            n_intermediate,
            vocab_size,
            rope_theta,
        }
    }
}

/// Layer-by-layer forward pass coordinator.
///
/// Mirrors AirLLM's `AirLLMBaseModel`: loads one layer at a time from disk,
/// runs the forward pass, then drops the layer to free RAM/VRAM.
pub struct StreamingForward {
    loader: Arc<LayerLoader>,
    global: GlobalWeights,
    config: ModelConfig,
    rope: Rope,
    inference_state: InferenceState,
}

impl StreamingForward {
    /// Create from an already-constructed loader and config.
    pub fn new(loader: LayerLoader, config: ModelConfig) -> Result<Self> {
        Self::new_shared(Arc::new(loader), config)
    }

    /// Create from a shared (Arc) loader — useful when the prefetcher also holds a reference.
    pub fn new_shared(loader: Arc<LayerLoader>, config: ModelConfig) -> Result<Self> {
        let global = loader.load_global()?;
        // max_seq_len: read from env or default to 512 to keep KV cache small.
        // KV cache size = n_layers × 2 × max_seq_len × n_kv_head × head_dim × 4 bytes.
        let max_seq_len: usize = std::env::var("MAX_SEQ_LEN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(512);
        let rope = Rope::new(config.head_dim, config.rope_theta, max_seq_len);
        let inference_state = InferenceState::new(
            config.n_layers,
            config.n_kv_head,
            config.head_dim,
            max_seq_len,
        );

        info!("✅ StreamingForward initialized: {} layers, {} embed dim, {}/{} heads",
            config.n_layers, config.n_embd, config.n_head, config.n_kv_head);

        Ok(Self {
            loader,
            global,
            config,
            rope,
            inference_state,
        })
    }

    /// Run forward pass for a single token, loading layers one at a time
    /// Returns logits: [vocab_size]
    pub fn forward_token(&mut self, token_id: usize) -> Result<Tensor> {
        let pos = self.inference_state.seq_len;

        // Step 1: Token embedding lookup + normalization scaling
        let h = self.embedding_lookup(token_id)?;

        // Step 2: Forward through layers (ONE AT A TIME from disk)
        let mut h = h;

        for layer_idx in 0..self.config.n_layers {
            info!("📦 Streaming layer {}/{} from disk", layer_idx, self.config.n_layers - 1);

            // Load only this layer from disk
            let layer = self.loader.load_layer(layer_idx)?;
            debug!("  Layer {} loaded: {} tensors, {:.1} MB",
                layer_idx,
                layer.tensors.len(),
                layer.size_bytes as f64 / (1024.0 * 1024.0)
            );

            // Run forward pass for this layer
            h = self.forward_layer(layer_idx, &h, pos, &layer)?;

            // Layer is dropped here (goes out of scope), freeing memory
            debug!("  Layer {} dropped from memory", layer_idx);
        }

        // Step 3: Final RMSNorm
        h = self.final_norm(&h)?;

        // Step 4: Output projection (lm_head)
        let logits = self.output_projection(&h)?;

        // Update sequence length
        self.inference_state.seq_len += 1;

        Ok(logits)
    }

    /// Embedding lookup: dequantize one row from the mmap (no full table in RAM).
    fn embedding_lookup(&self, token_id: usize) -> Result<Tensor> {
        let n_embd = self.config.n_embd;

        let mut h_data = self.loader.load_tensor_row("token_embd.weight", token_id)?;

        if h_data.len() != n_embd {
            anyhow::bail!(
                "embedding_lookup: got {} floats, expected n_embd={}",
                h_data.len(), n_embd
            );
        }

        // Gemma: scale embeddings by sqrt(n_embd)
        let scale = (n_embd as f32).sqrt();
        for x in h_data.iter_mut() {
            *x *= scale;
        }

        Ok(Tensor::new(h_data, vec![n_embd]))
    }

    /// Apply RMS norm independently to each head in a [total_dim] vector.
    ///
    /// Reshapes to [n_heads, head_dim], applies the norm weight per row, flattens back.
    fn per_head_norm(&self, x: &Tensor, n_heads: usize, head_dim: usize, norm_weight: &Tensor) -> Tensor {
        let x2d = Tensor::new(x.data.clone(), vec![n_heads, head_dim]);
        let normed = x2d.rms_norm(norm_weight, 1e-6);
        Tensor::new(normed.data, vec![x.nelems()])
    }

    /// Forward pass through a single transformer layer.
    ///
    /// Implements the Gemma 4 block:
    ///   h = h + PostAttnNorm(Attn(PreAttnNorm(h))) * layer_output_scale
    ///   h = h + PostFFNNorm(FFN(PreFFNNorm(h)))  * layer_output_scale
    ///
    /// Key Gemma 4 additions vs vanilla transformer:
    /// - Q/K per-head RMS normalization (before RoPE)
    /// - `layer_output_scale` scalar applied before each residual add
    fn forward_layer(&mut self, layer_idx: usize, h: &Tensor, pos: usize, layer: &LayerWeights) -> Result<Tensor> {
        // Per-layer output scale (Gemma 4 AltUp scalar)
        let output_scale: f32 = self.get_tensor(&layer.tensors, "layer_output_scale.weight")
            .map(|t| t.data[0])
            .unwrap_or(1.0);

        // ── Attention branch ──────────────────────────────────────────

        let attn_norm = self.get_tensor(&layer.tensors, "attn_norm.weight")?;
        let h_norm = h.rms_norm(attn_norm, 1e-6);

        let wq = self.get_tensor(&layer.tensors, "attn_q.weight")?;
        let q = self.matmul_1d(&h_norm, wq)?;
        let q_dim = q.nelems();

        let wk = self.get_tensor(&layer.tensors, "attn_k.weight")?;
        let k = self.matmul_1d(&h_norm, wk)?;
        let k_dim = k.nelems();

        let wv = self.get_tensor(&layer.tensors, "attn_v.weight")?;
        let v = self.matmul_1d(&h_norm, wv)?;

        let n_head   = self.config.n_head;
        let n_kv_head = self.config.n_kv_head;
        let actual_head_dim = q_dim / n_head;

        // ── Q/K per-head RMS normalization (Gemma 4, Llama 3.1+) ──────
        let q = if let Ok(qn) = self.get_tensor(&layer.tensors, "attn_q_norm.weight") {
            self.per_head_norm(&q, n_head, actual_head_dim, qn)
        } else { q };

        let k = if let Ok(kn) = self.get_tensor(&layer.tensors, "attn_k_norm.weight") {
            self.per_head_norm(&k, n_kv_head, actual_head_dim, kn)
        } else { k };

        // ── RoPE ──────────────────────────────────────────────────────
        let mut q_data = q.data.clone();
        self.rope.apply(&mut q_data, pos);
        let q = Tensor::new(q_data, vec![q_dim]);

        let mut k_data = k.data.clone();
        self.rope.apply(&mut k_data, pos);
        let k = Tensor::new(k_data, vec![k_dim]);

        // ── KV cache update ──────────────────────────────────────────
        let kv_offset = pos * k_dim;
        {
            let kv_cache = &mut self.inference_state.kv_cache[layer_idx];
            kv_cache.k_cache.data[kv_offset..kv_offset + k_dim].copy_from_slice(&k.data);
            kv_cache.v_cache.data[kv_offset..kv_offset + k_dim].copy_from_slice(&v.data);
            kv_cache.seq_len = pos + 1;
        }

        // ── Multi-head attention ──────────────────────────────────────
        let seq_len = pos + 1;
        let n_groups = n_head / n_kv_head;
        let q_2d = q.view_2d(n_head, actual_head_dim);
        let scale_inv_sqrt = 1.0 / (actual_head_dim as f32).sqrt();

        let mut attn_scores = vec![0.0f32; n_head * seq_len];
        for head_idx in 0..n_head {
            let kv_head_idx = head_idx / n_groups;
            let q_head = q_2d.get_row(head_idx);
            let kv_cache = &self.inference_state.kv_cache[layer_idx];
            for s in 0..seq_len {
                let k_base = s * k_dim + kv_head_idx * actual_head_dim;
                let score: f32 = q_head.data.iter()
                    .zip(&kv_cache.k_cache.data[k_base..k_base + actual_head_dim])
                    .map(|(a, b)| a * b)
                    .sum();
                attn_scores[head_idx * seq_len + s] = score * scale_inv_sqrt;
            }
        }

        let attn_2d = Tensor::new(attn_scores, vec![n_head, seq_len]).softmax();

        let attn_size = n_head * actual_head_dim;
        let mut attn_out = vec![0.0f32; attn_size];
        {
            let kv_cache = &self.inference_state.kv_cache[layer_idx];
            for head_idx in 0..n_head {
                let kv_head_idx = head_idx / n_groups;
                let attn_weights = &attn_2d.data[head_idx * seq_len..(head_idx + 1) * seq_len];
                for s in 0..seq_len {
                    let w = attn_weights[s];
                    let v_base = s * k_dim + kv_head_idx * actual_head_dim;
                    for d in 0..actual_head_dim {
                        attn_out[head_idx * actual_head_dim + d] +=
                            w * kv_cache.v_cache.data[v_base + d];
                    }
                }
            }
        }

        // Output projection: [q_dim] → [n_embd]
        let attn_out_tensor = Tensor::new(attn_out, vec![attn_size]);
        let wo = self.get_tensor(&layer.tensors, "attn_output.weight")?;
        let attn_proj = self.matmul_1d(&attn_out_tensor, wo)?;

        // Post-attention norm then scaled residual
        let attn_proj = if let Ok(pan) = self.get_tensor(&layer.tensors, "post_attention_norm.weight") {
            attn_proj.rms_norm(pan, 1e-6)
        } else { attn_proj };

        let h = h.add(&attn_proj.scale(output_scale));

        // ── Feed-forward branch ───────────────────────────────────────
        let ffn_norm = self.get_tensor(&layer.tensors, "ffn_norm.weight")?;
        let h_ffn_norm = h.rms_norm(ffn_norm, 1e-6);

        let ffn_gate = self.get_tensor(&layer.tensors, "ffn_gate.weight")?;
        let gate = self.matmul_1d(&h_ffn_norm, ffn_gate)?;

        let ffn_up = self.get_tensor(&layer.tensors, "ffn_up.weight")?;
        let up = self.matmul_1d(&h_ffn_norm, ffn_up)?;

        let ffn_intermediate = gate.gelu().mul(&up);

        let ffn_down = self.get_tensor(&layer.tensors, "ffn_down.weight")?;
        let ffn_out = self.matmul_1d(&ffn_intermediate, ffn_down)?;

        let ffn_out = if let Ok(pfw) = self.get_tensor(&layer.tensors, "post_ffw_norm.weight") {
            ffn_out.rms_norm(pfw, 1e-6)
        } else { ffn_out };

        let h_out = h.add(&ffn_out.scale(output_scale));

        Ok(h_out)
    }

    /// Final normalization before output projection
    fn final_norm(&self, h: &Tensor) -> Result<Tensor> {
        if let Some(norm_weight) = self.global.norms.get("output_norm.weight") {
            Ok(h.rms_norm(norm_weight, 1e-6))
        } else if let Some(norm_weight) = self.global.norms.get("norm.weight") {
            Ok(h.rms_norm(norm_weight, 1e-6))
        } else {
            // No normalization found, return as-is
            Ok(h.clone())
        }
    }

    /// Output projection: h · token_embd^T → logits [vocab_size].
    ///
    /// Uses tied embeddings (Gemma, Llama pattern): the same matrix used for
    /// token embedding is transposed for the output projection.
    /// Dequantizes one row at a time from the mmap — no large heap allocation.
    fn output_projection(&self, h: &Tensor) -> Result<Tensor> {
        // Try explicit output.weight first (some models have untied weights)
        if let Some(output_weight) = self.global.output.get("output.weight") {
            return self.matmul_1d(h, output_weight);
        }
        // Tied embeddings: compute logits row-by-row from mmap
        let logits = self.loader.compute_lm_head_logits(&h.data)?;
        let vocab_size = logits.len();
        Ok(Tensor::new(logits, vec![vocab_size]))
    }

    /// 1D matmul: [n] @ [n, m] -> [m]
    fn matmul_1d(&self, x: &Tensor, w: &Tensor) -> Result<Tensor> {
        let n = x.nelems();
        let m = w.cols();
        let w_data_len = w.nelems();

        // Debug: print matmul shapes
        debug!("matmul_1d: x=[{}] w.shape={:?} w_data=[{}] expected={}", 
            n, w.shape, w_data_len, n * m);

        let mut result = vec![0.0f32; m];

        let transposed = w_data_len != n * m;

        if transposed {
            for j in 0..m {
                for i in 0..n {
                    let idx = j * n + i;
                    if idx < w_data_len {
                        result[j] += x.data[i] * w.data[idx];
                    }
                }
            }
        } else {
            for j in 0..m {
                for i in 0..n {
                    let idx = i * m + j;
                    if idx < w_data_len {
                        result[j] += x.data[i] * w.data[idx];
                    }
                }
            }
        }

        Ok(Tensor::new(result, vec![m]))
    }

    /// Get a tensor from layer weights, error if not found
    fn get_tensor<'a>(&self, tensors: &'a HashMap<String, Tensor>, name: &str) -> Result<&'a Tensor> {
        // Exact match first
        if let Some(t) = tensors.get(name) {
            return Ok(t);
        }

        // Suffix match
        for (key, tensor) in tensors {
            if key.ends_with(name) {
                debug!("get_tensor: '{}' matched key='{}' shape={:?}", name, key, tensor.shape);
                return Ok(tensor);
            }
        }

        anyhow::bail!("Tensor '{}' not found in layer. Available: {}", name,
            tensors.keys().cloned().collect::<Vec<_>>().join(", "))
    }

    /// Get current sequence length
    pub fn seq_len(&self) -> usize {
        self.inference_state.seq_len
    }

    /// Get a reference to the model config
    pub fn config(&self) -> &ModelConfig {
        &self.config
    }

    /// Reset the inference state (KV cache).
    ///
    /// Call this between independent generation requests so that
    /// KV cache from one prompt does not bleed into the next.
    pub fn reset(&mut self) {
        self.inference_state = InferenceState::new(
            self.config.n_layers,
            self.config.n_kv_head,
            self.config.head_dim,
            self.inference_state.max_seq_len,
        );
    }

    /// Autoregressive text generation — the equivalent of AirLLM's `generate()`.
    ///
    /// ## Phases
    /// 1. **Prefill**: forward every prompt token through all layers, building the
    ///    KV cache.  Only the logits from the *last* prompt token are used.
    /// 2. **Decode**: sample the next token from logits, forward it, repeat until
    ///    `max_new_tokens` are generated or the `eos_token` is produced.
    ///
    /// ## Returns
    /// The **new** token IDs only (not the prompt).
    ///
    /// ## Memory behaviour
    /// Each call to `forward_token` loads a layer, runs it, then drops it —
    /// peak RAM stays at ~1 layer at a time, regardless of model size.
    pub fn generate(
        &mut self,
        prompt_tokens: &[u32],
        max_new_tokens: usize,
        temperature: f32,
        eos_token: u32,
    ) -> Result<Vec<u32>> {
        if prompt_tokens.is_empty() {
            anyhow::bail!("generate() called with empty prompt");
        }

        let strategy = if temperature <= 0.0 {
            SamplingStrategy::Greedy
        } else {
            SamplingStrategy::Random
        };

        let prompt_len = prompt_tokens.len();
        info!("generate: prefill {} tokens, decode up to {}", prompt_len, max_new_tokens);

        // --- Prefill phase ---
        let mut last_logits: Option<Tensor> = None;
        for (i, &token_id) in prompt_tokens.iter().enumerate() {
            let logits = self.forward_token(token_id as usize)?;
            if i == prompt_len - 1 {
                last_logits = Some(logits);
            }
        }

        let mut logits = last_logits
            .expect("prompt_tokens was non-empty but produced no logits");

        // --- Decode phase ---
        let mut output_tokens: Vec<u32> = Vec::with_capacity(max_new_tokens);

        for step in 0..max_new_tokens {
            let next_token = sample(&logits, strategy, temperature) as u32;

            if next_token == eos_token {
                info!("generate: EOS at step {}", step);
                break;
            }

            output_tokens.push(next_token);
            logits = self.forward_token(next_token as usize)?;
        }

        info!("generate: produced {} new tokens", output_tokens.len());
        Ok(output_tokens)
    }
}
