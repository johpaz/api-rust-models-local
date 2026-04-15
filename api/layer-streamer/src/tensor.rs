use std::fmt;

/// Basic f32 tensor with row-major layout
#[derive(Clone)]
pub struct Tensor {
    pub data: Vec<f32>,
    pub shape: Vec<usize>, // [rows, cols] for 2D, [n] for 1D
}

impl Tensor {
    pub fn new(data: Vec<f32>, shape: Vec<usize>) -> Self {
        let expected: usize = shape.iter().product();
        assert_eq!(data.len(), expected, "Tensor data len {} != shape product {}", data.len(), expected);
        Self { data, shape }
    }

    pub fn zeros(shape: &[usize]) -> Self {
        let len: usize = shape.iter().product();
        Self { data: vec![0.0; len], shape: shape.to_vec() }
    }

    pub fn from_2d(rows: usize, cols: usize) -> Self {
        Self::zeros(&[rows, cols])
    }

    pub fn from_1d(n: usize) -> Self {
        Self::zeros(&[n])
    }

    pub fn rows(&self) -> usize {
        if self.shape.len() >= 2 { self.shape[0] } else { 1 }
    }

    pub fn cols(&self) -> usize {
        if self.shape.len() >= 2 { self.shape[1] } else { self.shape[0] }
    }

    pub fn nelems(&self) -> usize {
        self.data.len()
    }

    pub fn is_1d(&self) -> bool {
        self.shape.len() == 1
    }

    pub fn is_2d(&self) -> bool {
        self.shape.len() >= 2
    }

    /// Matrix multiply: (M x K) @ (K x N) -> (M x N)
    pub fn matmul(&self, other: &Tensor) -> Tensor {
        let m = self.rows();
        let k = self.cols();
        let n = other.cols();

        assert_eq!(k, other.rows(), "Matmul shape mismatch: {}x{} @ {}x{}", m, k, other.rows(), n);

        let mut result = vec![0.0f32; m * n];

        // Naive O(M*N*K) matmul - optimized with loop reordering for cache
        for i in 0..m {
            for p in 0..k {
                let a = self.data[i * k + p];
                if a == 0.0 { continue; }
                for j in 0..n {
                    result[i * n + j] += a * other.data[p * n + j];
                }
            }
        }

        Tensor::new(result, vec![m, n])
    }

    /// Multiply each row by a 1D tensor (element-wise broadcast)
    pub fn mul_rowwise(&self, weights: &Tensor) -> Tensor {
        assert!(weights.is_1d(), "Weights must be 1D");
        let n = weights.nelems();
        assert_eq!(self.cols(), n, "Col count {} != weight size {}", self.cols(), n);

        let mut result = self.data.clone();
        for i in 0..self.rows() {
            for j in 0..n {
                result[i * n + j] *= weights.data[j];
            }
        }
        Tensor::new(result, self.shape.clone())
    }

    /// Add 1D tensor as bias to each row
    pub fn add_bias(&self, bias: &Tensor) -> Tensor {
        assert!(bias.is_1d(), "Bias must be 1D");
        let n = bias.nelems();
        assert_eq!(self.cols(), n, "Col count {} != bias size {}", self.cols(), n);

        let mut result = self.data.clone();
        for i in 0..self.rows() {
            for j in 0..n {
                result[i * n + j] += bias.data[j];
            }
        }
        Tensor::new(result, self.shape.clone())
    }

    /// Element-wise add: self + other
    pub fn add(&self, other: &Tensor) -> Tensor {
        assert_eq!(self.data.len(), other.data.len());
        let result: Vec<f32> = self.data.iter().zip(other.data.iter())
            .map(|(a, b)| a + b)
            .collect();
        Tensor::new(result, self.shape.clone())
    }

    /// Element-wise mul: self * other
    pub fn mul(&self, other: &Tensor) -> Tensor {
        assert_eq!(self.data.len(), other.data.len());
        let result: Vec<f32> = self.data.iter().zip(other.data.iter())
            .map(|(a, b)| a * b)
            .collect();
        Tensor::new(result, self.shape.clone())
    }

    /// Element-wise SiLU: x * sigmoid(x)
    pub fn silu(&self) -> Tensor {
        let result: Vec<f32> = self.data.iter()
            .map(|&x| {
                let sig = 1.0 / (1.0 + (-x).exp());
                x * sig
            })
            .collect();
        Tensor::new(result, self.shape.clone())
    }

    /// Element-wise GeLU approximation
    pub fn gelu(&self) -> Tensor {
        let result: Vec<f32> = self.data.iter()
            .map(|&x| {
                let c = 0.044715;
                x * 0.5 * (1.0 + ((2.0 / std::f32::consts::PI).sqrt() * (x + c * x * x * x)).tanh())
            })
            .collect();
        Tensor::new(result, self.shape.clone())
    }

    /// RMSNorm: x / sqrt(mean(x^2) + eps) * weight
    pub fn rms_norm(&self, weight: &Tensor, eps: f32) -> Tensor {
        // Handle both 1D and 2D weights
        let weight_data = if weight.is_1d() {
            weight.data.clone()
        } else {
            // Flatten 2D weight to 1D
            weight.data.clone()
        };
        let n = weight_data.len();
        let n_rows = if self.is_1d() { 1 } else { self.rows() };
        let cols = if self.is_1d() { self.nelems() } else { self.cols() };

        // If cols != n, we may need to handle transposed weights
        assert_eq!(cols, n, "Col count {} != weight size {} (self={:?}, weight={:?})",
            cols, n, self.shape, weight.shape);

        let mut result = vec![0.0f32; self.nelems()];

        for i in 0..n_rows {
            let row_start = i * n;
            let sum_sq: f32 = self.data[row_start..row_start + n]
                .iter()
                .map(|&x| x * x)
                .sum();
            let rms = (sum_sq / n as f32 + eps).sqrt();

            for j in 0..n {
                result[row_start + j] = self.data[row_start + j] / rms * weight_data[j];
            }
        }

        Tensor::new(result, self.shape.clone())
    }

    /// Softmax over last dimension
    pub fn softmax(&self) -> Tensor {
        let n_rows = if self.is_1d() { 1 } else { self.rows() };
        let n_cols = if self.is_1d() { self.nelems() } else { self.cols() };
        let mut result = self.data.clone();

        for i in 0..n_rows {
            let start = i * n_cols;
            let row = &mut result[start..start + n_cols];

            // Stable softmax
            let max_val = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let sum: f32 = row.iter()
                .map(|&x| (x - max_val).exp())
                .sum();
            let log_sum = max_val + sum.ln();

            for x in row.iter_mut() {
                *x = (*x - log_sum).exp();
            }
        }

        Tensor::new(result, self.shape.clone())
    }

    /// Transpose a 2D tensor
    pub fn transpose(&self) -> Tensor {
        assert!(self.is_2d());
        let m = self.rows();
        let n = self.cols();
        let mut result = vec![0.0f32; m * n];

        for i in 0..m {
            for j in 0..n {
                result[j * m + i] = self.data[i * n + j];
            }
        }

        Tensor::new(result, vec![n, m])
    }

    /// Get a single row as a 1D tensor
    pub fn get_row(&self, idx: usize) -> Tensor {
        assert!(self.is_2d());
        assert!(idx < self.rows());
        let n = self.cols();
        let start = idx * n;
        Tensor::new(self.data[start..start + n].to_vec(), vec![n])
    }

    /// Get scalar value from 1D tensor at index
    pub fn get_scalar(&self, idx: usize) -> f32 {
        assert!(self.is_1d());
        self.data[idx]
    }

    /// Argmax: return index of max value
    pub fn argmax(&self) -> usize {
        assert!(self.is_1d());
        self.data.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap()
    }

    /// Reshape to new shape (same total elements)
    pub fn reshape(&self, new_shape: Vec<usize>) -> Tensor {
        let expected: usize = new_shape.iter().product();
        assert_eq!(self.nelems(), expected, "Cannot reshape {} to {:?}", self.nelems(), new_shape);
        Tensor {
            data: self.data.clone(),
            shape: new_shape,
        }
    }

    /// View as 2D with given shape (same data, new shape)
    pub fn view_2d(&self, rows: usize, cols: usize) -> Tensor {
        assert_eq!(self.nelems(), rows * cols);
        Tensor {
            data: self.data.clone(),
            shape: vec![rows, cols],
        }
    }

    /// Concatenate along first dimension
    pub fn concat_rows(tensors: &[&Tensor]) -> Tensor {
        assert!(!tensors.is_empty());
        let n_cols = tensors[0].cols();
        let n_rows: usize = tensors.iter().map(|t| t.rows()).sum();

        let mut data = Vec::with_capacity(n_rows * n_cols);
        for t in tensors {
            assert_eq!(t.cols(), n_cols);
            data.extend_from_slice(&t.data);
        }

        Tensor::new(data, vec![n_rows, n_cols])
    }

    /// Scale all elements by a scalar
    pub fn scale(&self, s: f32) -> Tensor {
        Tensor::new(
            self.data.iter().map(|&x| x * s).collect(),
            self.shape.clone(),
        )
    }

    pub fn fmt_short(&self) -> String {
        format!("Tensor{:?} [{} elems]", self.shape, self.nelems())
    }
}

impl fmt::Debug for Tensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tensor{:?} ({} elems, {:.1} MB)",
            self.shape,
            self.nelems(),
            self.nelems() as f64 * 4.0 / (1024.0 * 1024.0)
        )
    }
}
