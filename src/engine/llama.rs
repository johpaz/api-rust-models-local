use crate::config::Config;
use crate::error::AppError;
use llama_cpp_2::{
    context::LlamaContext,
    model::LlamaModel,
    token::data_array::LlamaTokenDataArray,
    token::LlamaToken,
};
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::sampling::LlamaSampler;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Semaphore};
use tracing::{info, error};

pub struct LlamaEngine {
    model: LlamaModel,
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
        let model = LlamaModel::load_from_file(&backend, &config.model_path, &model_params)
            .map_err(|e| AppError::ModelLoadError(e.to_string()))?;

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
        stop: Vec<String>,
    ) -> Result<mpsc::Receiver<String>, AppError> {
        let _permit = self.semaphore.acquire().await.map_err(|e| AppError::Internal(e.to_string()))?;

        let mut context_params = LlamaContextParams::default();
        context_params.set_n_ctx(NonZeroU32::new(self.context_size).unwrap());

        let mut context = self.model
            .new_context(&self.backend, context_params)
            .map_err(|e| AppError::EngineError(e.to_string()))?;

        let tokens_list = self.model
            .str_to_token(&prompt, llama_cpp_2::model::AddBos::Always)
            .map_err(|e| AppError::EngineError(e.to_string()))?;

        let n_ctx = context.n_ctx() as usize;
        let n_kv_max = n_ctx;
        let n_tokens = tokens_list.len();

        if n_tokens > n_kv_max {
            return Err(AppError::InvalidRequest(format!(
                "Prompt is too long ({} tokens, max {})",
                n_tokens, n_kv_max
            )));
        }

        let mut batch = llama_cpp_2::batch::LlamaBatch::new(n_tokens, 1, 1);
        for (i, token) in tokens_list.iter().enumerate() {
            batch.add(*token, i as i32, &[0], i == n_tokens - 1);
        }

        context.decode(&mut batch).map_err(|e| AppError::EngineError(e.to_string()))?;

        let (tx, rx) = mpsc::channel(100);
        let model = self.model.clone();
        let backend = self.backend.clone();

        tokio::spawn(async move {
            let mut sampler = LlamaSampler::chain_simple([
                LlamaSampler::temp(temperature),
                LlamaSampler::top_k(40),
                LlamaSampler::top_p(0.95, 1),
            ]);

            let mut n_cur = n_tokens;
            while n_cur < n_kv_max && n_cur - n_tokens < max_tokens {
                let token = sampler.sample(&context, batch.n_tokens() - 1);
                sampler.accept(token);

                if model.is_eot_token(token) {
                    break;
                }

                let output_bytes = model.token_to_str(token).unwrap_or_default();
                if tx.send(output_bytes).await.is_err() {
                    break;
                }

                batch.clear();
                batch.add(token, n_cur as i32, &[0], true);
                if let Err(e) = context.decode(&mut batch) {
                    error!("Decoding error: {}", e);
                    break;
                }
                n_cur += 1;
            }
            drop(_permit);
        });

        Ok(rx)
    }
}
