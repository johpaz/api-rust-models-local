/// Rotary Position Embedding (RoPE)
/// Applied to Q and K tensors before attention computation.
pub struct Rope {
    dim: usize,
    theta: f32,
    /// Precomputed cos/sin tables: [max_seq_len][dim/2]
    cos_cache: Vec<Vec<f32>>,
    sin_cache: Vec<Vec<f32>>,
}

impl Rope {
    pub fn new(dim: usize, theta: f32, max_seq_len: usize) -> Self {
        let half_dim = dim / 2;
        let mut cos_cache = Vec::with_capacity(max_seq_len);
        let mut sin_cache = Vec::with_capacity(max_seq_len);

        for pos in 0..max_seq_len {
            let mut cos_row = Vec::with_capacity(half_dim);
            let mut sin_row = Vec::with_capacity(half_dim);

            for i in 0..half_dim {
                let freq = 1.0 / theta.powf(i as f32 / dim as f32);
                let theta_val = pos as f32 * freq;
                cos_row.push(theta_val.cos());
                sin_row.push(theta_val.sin());
            }

            cos_cache.push(cos_row);
            sin_cache.push(sin_row);
        }

        Self { dim, theta, cos_cache, sin_cache }
    }

    /// Create RoPE with default theta for the model
    pub fn from_config(dim: usize, max_seq_len: usize) -> Self {
        // Default theta: 10000.0 for most models
        Self::new(dim, 10000.0, max_seq_len)
    }

    /// Apply RoPE to a 1D tensor representing [dim] at a given position
    /// The tensor should be of size `dim`
    pub fn apply(&self, data: &mut [f32], pos: usize) {
        let half_dim = self.dim / 2;

        if pos >= self.cos_cache.len() {
            // Extend cache if needed
            return;
        }

        let cos = &self.cos_cache[pos];
        let sin = &self.sin_cache[pos];

        for i in 0..half_dim {
            let x0 = data[i * 2];
            let x1 = data[i * 2 + 1];
            let c = cos[i];
            let s = sin[i];

            // RoPE: [x0, x1] -> [x0*c - x1*s, x0*s + x1*c]
            data[i * 2] = x0 * c - x1 * s;
            data[i * 2 + 1] = x0 * s + x1 * c;
        }
    }

    /// Apply RoPE to a 2D tensor [n_heads * seq_len][head_dim] at given positions
    pub fn apply_2d(&self, data: &mut [f32], n_rows: usize, head_dim: usize, start_pos: usize) {
        assert_eq!(data.len(), n_rows * head_dim);
        assert_eq!(head_dim % 2, 0, "head_dim must be even for RoPE");

        let half_dim = head_dim / 2;

        for row in 0..n_rows {
            let pos = start_pos + row;
            if pos >= self.cos_cache.len() {
                continue;
            }

            let cos = &self.cos_cache[pos];
            let sin = &self.sin_cache[pos];
            let row_start = row * head_dim;

            for i in 0..half_dim {
                let x0 = data[row_start + i * 2];
                let x1 = data[row_start + i * 2 + 1];
                let c = cos[i];
                let s = sin[i];

                data[row_start + i * 2] = x0 * c - x1 * s;
                data[row_start + i * 2 + 1] = x0 * s + x1 * c;
            }
        }
    }

    /// Get max sequence length
    pub fn max_seq_len(&self) -> usize {
        self.cos_cache.len()
    }
}
