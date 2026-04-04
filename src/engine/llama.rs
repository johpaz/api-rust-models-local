use crate::config::Config;
use crate::error::AppError;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::sampling::LlamaSampler;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{info, error};

pub struct LlamaEngine {
    model: Arc<LlamaModel>,
    semaphore: Arc<Semaphore>,
    backend: Arc<LlamaBackend>,
    context_size: u32,
}

impl LlamaEngine {
    pub async fn new(config: Config) -> Result<Self, AppError> {
        let backend = Arc::new(LlamaBackend::init().map_err(|e| AppError::ModelLoadError(e.to_string()))?);

        if !Path::new(&config.model_path).exists() {
            return Err(AppError::ModelLoadError(format!("Model file not found at {}", config.model_path)));
        }

        let model_params = LlamaModelParams::default();
        let model = Arc::new(
            LlamaModel::load_from_file(&backend, &config.model_path, &model_params)
                .map_err(|e| AppError::ModelLoadError(e.to_string()))?,
        );

        info!("Model loaded successfully from {}", config.model_path);

        Ok(Self {
            model,
            semaphore: Arc::new(Semaphore::new(config.max_concurrency)),
            backend,
            context_size: config.context_size,
        })
    }

    pub fn get_model_name(&self) -> String {
        "local-gguf-model".to_string()
    }

    pub async fn generate_stream(
        &self,
        prompt: String,
        temperature: f32,
        max_tokens: usize,
        _stop: Vec<String>,
    ) -> Result<mpsc::Receiver<String>, AppError> {
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let model = Arc::clone(&self.model);
        let backend = Arc::clone(&self.backend);
        let context_size = self.context_size;
        let (tx, rx) = mpsc::channel(100);

        tokio::task::spawn_blocking(move || {
            let _permit = permit;

            let context_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(context_size));

            let mut context = match model.new_context(&backend, context_params) {
                Ok(ctx) => ctx,
                Err(e) => {
                    let _ = tx.blocking_send(format!("[ERROR: {}]", e));
                    return;
                }
            };

            let tokens_list = match model.str_to_token(&prompt, llama_cpp_2::model::AddBos::Always) {
                Ok(t) => t,
                Err(e) => {
                    let _ = tx.blocking_send(format!("[ERROR: {}]", e));
                    return;
                }
            };

            let n_ctx = context.n_ctx() as usize;
            let n_tokens = tokens_list.len();

            if n_tokens > n_ctx {
                let _ = tx.blocking_send(format!(
                    "[ERROR: Prompt too long ({} tokens, max {})]",
                    n_tokens, n_ctx
                ));
                return;
            }

            let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(n_tokens, 1);
            for (i, token) in tokens_list.iter().enumerate() {
                if let Err(e) = batch.add(*token, i as i32, &[0], i == n_tokens - 1) {
                    let _ = tx.blocking_send(format!("[ERROR: batch add: {}]", e));
                    return;
                }
            }

            if let Err(e) = context.decode(&mut batch) {
                let _ = tx.blocking_send(format!("[ERROR: decode: {}]", e));
                return;
            }

            let mut sampler = LlamaSampler::chain_simple([
                LlamaSampler::temp(temperature),
                LlamaSampler::top_k(40),
                LlamaSampler::top_p(0.95, 1),
            ]);

            let mut n_cur = n_tokens;
            while n_cur < n_ctx && n_cur - n_tokens < max_tokens {
                let token = sampler.sample(&context, batch.n_tokens() - 1);
                sampler.accept(token);

                if model.is_eog_token(token) {
                    break;
                }

                let output_bytes = model.token_to_str(token, llama_cpp_2::model::Special::Tokenize).unwrap_or_default();
                if tx.blocking_send(output_bytes).is_err() {
                    break;
                }

                batch.clear();
                if let Err(e) = batch.add(token, n_cur as i32, &[0], true) {
                    error!("Batch add error: {}", e);
                    break;
                }
                if let Err(e) = context.decode(&mut batch) {
                    error!("Decoding error: {}", e);
                    break;
                }
                n_cur += 1;
            }
        });

        Ok(rx)
    }
}
