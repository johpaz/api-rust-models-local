use crate::tensor::Tensor;
use rand::Rng;

/// Sampling strategy for token generation
#[derive(Clone, Copy, Debug)]
pub enum SamplingStrategy {
    /// Always pick the most likely token
    Greedy,
    /// Random sample from full distribution
    Random,
}

/// Sample a token ID from logits
pub fn sample(logits: &Tensor, strategy: SamplingStrategy, temperature: f32) -> usize {
    match strategy {
        SamplingStrategy::Greedy => greedy_sample(logits),
        SamplingStrategy::Random => random_sample(logits, temperature),
    }
}

/// Greedy sampling: return the token with highest logit
pub fn greedy_sample(logits: &Tensor) -> usize {
    logits.argmax()
}

/// Random sampling with temperature scaling
pub fn random_sample(logits: &Tensor, temperature: f32) -> usize {
    assert!(logits.is_1d());
    let n = logits.nelems();

    // Apply temperature
    let temp = if temperature > 0.0 { temperature } else { 0.1 };
    let mut probs: Vec<f64> = Vec::with_capacity(n);
    let mut max_logit = f32::NEG_INFINITY;
    for &x in &logits.data {
        if x > max_logit { max_logit = x; }
    }

    // Softmax with temperature (stable)
    let mut sum = 0.0;
    for &x in &logits.data {
        let p = ((x - max_logit) / temp).exp() as f64;
        probs.push(p);
        sum += p;
    }

    // Normalize
    for p in probs.iter_mut() {
        *p /= sum;
    }

    // Sample from categorical distribution
    let mut rng = rand::thread_rng();
    let r = rng.gen::<f64>();

    let mut cumulative = 0.0;
    for i in 0..n {
        cumulative += probs[i];
        if r <= cumulative {
            return i;
        }
    }

    n - 1 // Fallback
}

/// Decode a token ID to string (simple lookup)
/// In practice, this needs a tokenizer/vocabulary
pub struct TokenDecoder {
    pub vocab: Vec<String>,
}

impl TokenDecoder {
    pub fn new(vocab: Vec<String>) -> Self {
        Self { vocab }
    }

    pub fn decode(&self, token_id: usize) -> &str {
        if token_id < self.vocab.len() {
            &self.vocab[token_id]
        } else {
            "<unk>"
        }
    }
}
