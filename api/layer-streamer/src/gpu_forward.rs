use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

use crate::vulkan_context::VulkanContext;
use crate::layer_loader::{LayerLoader, LayerWeights};
use crate::tensor::Tensor;
use crate::gpu_tensor::{GpuTensor, GpuContext};
use crate::gpu_ops::GpuOps;
use crate::rope::Rope;
use crate::forward::{InferenceState, GemmaConfig};

/// GPU-accelerated layer-by-layer forward pass
pub struct GpuForward {
    loader: LayerLoader,
    config: GemmaConfig,
    gpu_ctx: GpuContext,
    gpu_ops: GpuOps,
    rope: Rope,
    inference_state: InferenceState,
}

impl GpuForward {
    pub fn new(loader: LayerLoader, config: GemmaConfig) -> Result<Self> {
        info!("🔧 Creating GPU forward with Vulkan context...");

        let ctx = Arc::new(VulkanContext::new()?);
        let gpu_ops = GpuOps::new(ctx.clone());
        let gpu_ctx = GpuContext::new(&ctx);

        let rope = Rope::new(config.head_dim, config.rope_theta, 8192);
        let inference_state = InferenceState::new(
            config.n_layers,
            config.n_kv_head,
            config.head_dim,
            8192,
        );

        let has_gpu_pipelines = gpu_ops.has_gpu_pipelines();

        info!(
            "✅ GPU Forward ready: {} layers, {} embed, {} heads, GPU shaders={}",
            config.n_layers, config.n_embd, config.n_head, has_gpu_pipelines
        );

        Ok(Self {
            loader,
            config,
            gpu_ctx,
            gpu_ops,
            rope,
            inference_state,
        })
    }

    /// Run forward pass for a single token, loading layers one at a time
    /// All compute happens on GPU, only layer loading from disk on CPU
    pub fn forward_token_gpu(&mut self, token_id: usize) -> Result<Tensor> {
        let pos = self.inference_state.seq_len;
        let n_embd = self.config.n_embd;

        // Step 1: Embedding lookup + upload to GPU
        let h_gpu = self.embedding_lookup_gpu(token_id)?;

        // Step 2: Forward through layers (one at a time from disk)
        let mut h_gpu = h_gpu;

        for layer_idx in 0..self.config.n_layers {
            info!("📦 GPU: Streaming layer {}/{} from disk", layer_idx, self.config.n_layers - 1);

            // Load only this layer from disk
            let layer = self.loader.load_layer(layer_idx)?;
            debug!(
                "  Layer {} loaded: {} tensors, {:.1} MB",
                layer_idx,
                layer.tensors.len(),
                layer.size_bytes as f64 / (1024.0 * 1024.0)
            );

            // Upload weights to GPU
            let layer_gpu = self.upload_layer_to_gpu(&layer)?;

            // Run forward pass for this layer on GPU
            h_gpu = self.forward_layer_gpu(layer_idx, &h_gpu, pos, &layer_gpu)?;

            debug!("  Layer {} dropped from memory", layer_idx);
        }

        // Step 3: Final RMSNorm on GPU
        let h_gpu = self.final_norm_gpu(&h_gpu)?;

        // Step 4: Output projection on GPU
        let logits_gpu = self.output_projection_gpu(&h_gpu)?;

        // Download result to CPU
        let logits = logits_gpu.to_cpu()?;

        // Update sequence length
        self.inference_state.seq_len += 1;

        Ok(logits)
    }

    /// Embedding lookup on GPU
    fn embedding_lookup_gpu(&self, token_id: usize) -> Result<GpuTensor> {
        let embd = self.loader.load_global()?;
        let embd_tensor = embd.token_embd.get("token_embd.weight")
            .ok_or_else(|| anyhow::anyhow!("token_embd.weight not found"))?;

        let n_embd = self.config.n_embd;
        let start = token_id * n_embd;
        let mut h_data = embd_tensor.data[start..start + n_embd].to_vec();

        // Gemma: scale embeddings by sqrt(n_embd)
        let scale = (n_embd as f32).sqrt();
        for x in h_data.iter_mut() {
            *x *= scale;
        }

        let h_cpu = Tensor::new(h_data, vec![n_embd]);
        GpuTensor::from_tensor(&self.gpu_ctx, &h_cpu)
    }

    /// Upload layer weights to GPU
    fn upload_layer_to_gpu(&self, layer: &LayerWeights) -> Result<HashMap<String, GpuTensor>> {
        let mut gpu_tensors = HashMap::new();

        for (name, tensor) in &layer.tensors {
            let gpu_tensor = GpuTensor::from_tensor(&self.gpu_ctx, tensor)?;
            gpu_tensors.insert(name.clone(), gpu_tensor);
        }

        Ok(gpu_tensors)
    }

    /// Forward pass through a single layer on GPU
    fn forward_layer_gpu(
        &mut self,
        layer_idx: usize,
        h: &GpuTensor,
        pos: usize,
        layer: &HashMap<String, GpuTensor>,
    ) -> Result<GpuTensor> {
        let n_embd = self.config.n_embd;
        let n_head = self.config.n_head;
        let n_kv_head = self.config.n_kv_head;
        let head_dim = self.config.head_dim;

        // --- Attention branch ---

        // RMSNorm
        let attn_norm = self.get_gpu_tensor(layer, "attn_norm.weight")?;
        let h_norm = self.gpu_ops.rms_norm_gpu(h, attn_norm, 1e-6)?;

        // Q projection: [n_embd] @ [n_embd, n_embd] -> [n_embd]
        let wq = self.get_gpu_tensor(layer, "attn_q.weight")?;
        let q = self.gpu_ops.matmul_gpu(&h_norm, wq)?;

        // K projection
        let wk = self.get_gpu_tensor(layer, "attn_k.weight")?;
        let k = self.gpu_ops.matmul_gpu(&h_norm, wk)?;

        // V projection
        let wv = self.get_gpu_tensor(layer, "attn_v.weight")?;
        let v = self.gpu_ops.matmul_gpu(&h_norm, wv)?;

        // Apply RoPE (CPU for now, needs GPU shader)
        let mut q_data = q.to_cpu()?.data;
        self.rope.apply(&mut q_data, pos);
        let q = GpuTensor::from_tensor(&self.gpu_ctx, &Tensor::new(q_data, vec![n_embd]))?;

        let mut k_data = k.to_cpu()?.data;
        self.rope.apply(&mut k_data, pos);
        let k = GpuTensor::from_tensor(&self.gpu_ctx, &Tensor::new(k_data, vec![n_kv_head * head_dim]))?;

        // Update KV cache (CPU for now)
        let kv_offset = pos * n_kv_head * head_dim;
        let kv_cache = &mut self.inference_state.kv_cache[layer_idx];
        let k_cpu = k.to_cpu()?;
        let v_cpu = v.to_cpu()?;
        for i in 0..k_cpu.nelems() {
            kv_cache.k_cache.data[kv_offset + i] = k_cpu.data[i];
            kv_cache.v_cache.data[kv_offset + i] = v_cpu.data[i];
        }
        kv_cache.seq_len = pos + 1;

        // Multi-query attention
        let seq_len = pos + 1;
        let n_groups = n_head / n_kv_head;

        // Compute attention on CPU (needs GPU attention shader)
        let q_cpu = q.to_cpu()?;
        let q_2d = q_cpu.view_2d(n_head, head_dim);

        let mut attn_output = vec![0.0f32; n_head * seq_len];
        for head_idx in 0..n_head {
            let kv_head_idx = head_idx / n_groups;
            let q_head = q_2d.get_row(head_idx);

            for s in 0..seq_len {
                let mut score = 0.0;
                for d in 0..head_dim {
                    let k_val = kv_cache.k_cache.data[s * n_kv_head * head_dim + kv_head_idx * head_dim + d];
                    score += q_head.data[d] * k_val;
                }
                attn_output[head_idx * seq_len + s] = score / (head_dim as f32).sqrt();
            }
        }

        // Softmax on CPU
        let mut attn_2d = Tensor::new(attn_output.clone(), vec![n_head, seq_len]);
        attn_2d = attn_2d.softmax();

        // Weighted sum of V
        let mut attn_out = vec![0.0f32; n_embd];
        for head_idx in 0..n_head {
            let kv_head_idx = head_idx / n_groups;
            let attn_weights = &attn_2d.data[head_idx * seq_len..(head_idx + 1) * seq_len];

            for s in 0..seq_len {
                let w = attn_weights[s];
                for d in 0..head_dim {
                    let v_val = kv_cache.v_cache.data[s * n_kv_head * head_dim + kv_head_idx * head_dim + d];
                    attn_out[head_idx * head_dim + d] += w * v_val;
                }
            }
        }

        let attn_out_tensor = Tensor::new(attn_out, vec![n_embd]);
        let attn_out_gpu = GpuTensor::from_tensor(&self.gpu_ctx, &attn_out_tensor)?;

        // Output projection on GPU
        let wo = self.get_gpu_tensor(layer, "attn_output.weight")?;
        let attn_proj = self.gpu_ops.matmul_gpu(&attn_out_gpu, wo)?;

        // Post-attention normalization on GPU
        let post_attn_norm = self.get_gpu_tensor(layer, "post_attention_norm.weight")?;
        let attn_proj = self.gpu_ops.rms_norm_gpu(&attn_proj, post_attn_norm, 1e-6)?;

        // Residual connection: h + attn_proj
        let attn_proj = self.gpu_ops.add_gpu(h, &attn_proj)?;

        // --- FFN branch ---

        // RMSNorm
        let ffn_norm = self.get_gpu_tensor(layer, "ffn_norm.weight")?;
        let h_ffn_norm = self.gpu_ops.rms_norm_gpu(&attn_proj, ffn_norm, 1e-6)?;

        // Gate projection on GPU
        let ffn_gate = self.get_gpu_tensor(layer, "ffn_gate.weight")?;
        let gate = self.gpu_ops.matmul_gpu(&h_ffn_norm, ffn_gate)?;

        // Up projection on GPU
        let ffn_up = self.get_gpu_tensor(layer, "ffn_up.weight")?;
        let up = self.gpu_ops.matmul_gpu(&h_ffn_norm, ffn_up)?;

        // SiLU activation on GPU
        let gate_activated = self.gpu_ops.silu_gpu(&gate)?;

        // Element-wise multiply on GPU
        let ffn_intermediate = self.gpu_ops.mul_gpu(&gate_activated, &up)?;

        // Down projection on GPU
        let ffn_down = self.get_gpu_tensor(layer, "ffn_down.weight")?;
        let ffn_out = self.gpu_ops.matmul_gpu(&ffn_intermediate, ffn_down)?;

        // Post-FFW normalization on GPU
        let post_ffw_norm = self.get_gpu_tensor(layer, "post_ffw_norm.weight")?;
        let ffn_out = self.gpu_ops.rms_norm_gpu(&ffn_out, post_ffw_norm, 1e-6)?;

        // Residual connection on GPU
        let h_out = self.gpu_ops.add_gpu(&attn_proj, &ffn_out)?;

        // Post normalization (Gemma 4 specific)
        if let Ok(post_norm) = self.get_gpu_tensor(layer, "post_norm.weight") {
            let h_normalized = self.gpu_ops.rms_norm_gpu(&h_out, post_norm, 1e-6)?;
            Ok(h_normalized)
        } else {
            Ok(h_out)
        }
    }

    /// Final normalization before output projection
    fn final_norm_gpu(&self, h: &GpuTensor) -> Result<GpuTensor> {
        let global = self.loader.load_global()?;

        if let Some(norm_weight) = global.norms.get("output_norm.weight") {
            let norm_gpu = GpuTensor::from_tensor(&self.gpu_ctx, norm_weight)?;
            self.gpu_ops.rms_norm_gpu(&h, &norm_gpu, 1e-6)
        } else if let Some(norm_weight) = global.norms.get("norm.weight") {
            let norm_gpu = GpuTensor::from_tensor(&self.gpu_ctx, norm_weight)?;
            self.gpu_ops.rms_norm_gpu(&h, &norm_gpu, 1e-6)
        } else {
            // No normalization, return as-is - need to create a new GPU tensor
            let h_cpu = h.to_cpu()?;
            GpuTensor::from_tensor(&self.gpu_ctx, &h_cpu)
        }
    }

    /// Output projection on GPU
    fn output_projection_gpu(&self, h: &GpuTensor) -> Result<GpuTensor> {
        let global = self.loader.load_global()?;

        if let Some(output_weight) = global.output.get("output.weight") {
            let output_gpu = GpuTensor::from_tensor(&self.gpu_ctx, output_weight)?;
            self.gpu_ops.matmul_gpu(h, &output_gpu)
        } else {
            anyhow::bail!("No output projection found")
        }
    }

    /// Get a GPU tensor from layer weights
    fn get_gpu_tensor<'a>(&self, tensors: &'a HashMap<String, GpuTensor>, name: &str) -> Result<&'a GpuTensor> {
        if let Some(t) = tensors.get(name) {
            return Ok(t);
        }

        for (key, tensor) in tensors {
            if key.ends_with(name) {
                return Ok(tensor);
            }
        }

        anyhow::bail!("GPU tensor '{}' not found. Available: {}", name,
            tensors.keys().cloned().collect::<Vec<_>>().join(", "))
    }

    /// Get current sequence length
    pub fn seq_len(&self) -> usize {
        self.inference_state.seq_len
    }
}
