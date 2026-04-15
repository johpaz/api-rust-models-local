//! Inference backend — patrón Actor (AirLLM methodology).
//!
//! `InferenceActor` vive en un `std::thread` dedicado que es dueño exclusivo
//! de `StreamingForward`. Los handlers async se comunican con él via canales
//! Tokio, sin ningún Mutex en código async y sin bloquear el runtime.
//!
//! Flujo por request:
//!   Handler async  ──mpsc──▶  Actor thread  ──oneshot/channel──▶  Handler async
//!                              (dueño de StreamingForward)

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use layer_streamer::{
    parse_gguf, GGUFTokenizer, LayerLoader, ModelConfig, StreamingForward,
};

// ─────────────────────────────────────────────────────────────────────────────
// Request / Response types (OpenAI-compatible subset)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub model: String,
    pub prompt: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub model: String,
    pub text: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Mensajes internos del actor
// ─────────────────────────────────────────────────────────────────────────────

enum InferenceMsg {
    /// Completar todo el texto de una vez (respuesta batch).
    Complete {
        req: CompletionRequest,
        reply: oneshot::Sender<Result<CompletionResponse>>,
    },
    /// Streaming token a token. Cada token decodificado se envía como `Some(text)`.
    /// Al terminar se envía `None` como señal de fin.
    Stream {
        req: CompletionRequest,
        token_tx: mpsc::UnboundedSender<Option<String>>,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Actor handle (lo que guarda AppState)
// ─────────────────────────────────────────────────────────────────────────────

/// Handle público para comunicarse con el thread de inferencia.
///
/// Los metadatos del modelo (`model_name`, `vocab_size`, `n_layers`) son
/// campos directos para que `stream_status` los lea sin ningún lock.
pub struct InferenceActor {
    sender: mpsc::Sender<InferenceMsg>,
    pub model_name: String,
    pub vocab_size: usize,
    pub n_layers: usize,
}

impl InferenceActor {
    /// Carga el modelo y lanza el thread del actor.
    ///
    /// La carga pesada ocurre dentro de `spawn_blocking` para no bloquear
    /// el runtime Tokio.
    pub async fn load(model_path: &str, layers_dir: &str) -> Result<Self> {
        let model_path_s = model_path.to_string();
        let layers_dir_s = layers_dir.to_string();

        // Cargar modelo en thread bloqueante (puede tardar varios segundos)
        let (forward, tokenizer, config, model_name) =
            tokio::task::spawn_blocking(move || -> Result<_> {
                let mp = PathBuf::from(&model_path_s);
                let ld = PathBuf::from(&layers_dir_s);

                tracing::info!("InferenceActor: cargando {} desde {}", mp.display(), ld.display());

                let model_info = parse_gguf(&mp)?;
                let config = ModelConfig::from_gguf(&model_info);
                let tokenizer = Arc::new(GGUFTokenizer::from_model_info(&model_info)?);
                let loader = LayerLoader::new(&ld, &mp)?;
                let forward = StreamingForward::new(loader, config.clone())?;

                let name = mp
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                Ok((forward, tokenizer, config, name))
            })
            .await??;

        let vocab_size = tokenizer.vocab_size();
        let n_layers = config.n_layers;

        tracing::info!(
            "InferenceActor listo: model={}, vocab={}, layers={}",
            model_name, vocab_size, n_layers
        );

        // Canal con capacidad 8: si hay más de 8 requests en cola el sender
        // devuelve error, evitando que se acumule trabajo sin límite.
        let (sender, receiver) = mpsc::channel::<InferenceMsg>(8);

        // Lanzar el thread del actor (dueño exclusivo de forward + tokenizer)
        tokio::task::spawn_blocking(move || {
            run_actor(forward, tokenizer, receiver);
        });

        Ok(Self {
            sender,
            model_name,
            vocab_size,
            n_layers,
        })
    }

    /// Completar un prompt y esperar todos los tokens (modo batch).
    ///
    /// No bloquea el runtime: envía el mensaje al actor y espera la respuesta
    /// via `oneshot`.
    pub async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.sender
            .send(InferenceMsg::Complete { req, reply: reply_tx })
            .await
            .map_err(|_| anyhow::anyhow!("Actor thread terminó inesperadamente"))?;

        reply_rx
            .await
            .map_err(|_| anyhow::anyhow!("Actor no respondió al Complete"))?
    }

    /// Iniciar streaming token a token.
    ///
    /// Devuelve un `UnboundedReceiver<Option<String>>`:
    /// - `Some(text)` → fragmento de texto decodificado
    /// - `None`       → fin del stream
    pub async fn stream(
        &self,
        req: CompletionRequest,
    ) -> Result<mpsc::UnboundedReceiver<Option<String>>> {
        let (token_tx, token_rx) = mpsc::unbounded_channel();
        self.sender
            .send(InferenceMsg::Stream { req, token_tx })
            .await
            .map_err(|_| anyhow::anyhow!("Actor thread terminó inesperadamente"))?;
        Ok(token_rx)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bucle del actor (corre en std::thread via spawn_blocking)
// ─────────────────────────────────────────────────────────────────────────────

fn run_actor(
    mut forward: StreamingForward,
    tokenizer: Arc<GGUFTokenizer>,
    mut rx: mpsc::Receiver<InferenceMsg>,
) {
    tracing::info!("InferenceActor thread arrancado");

    while let Some(msg) = rx.blocking_recv() {
        match msg {
            InferenceMsg::Complete { req, reply } => {
                let result = do_complete(&mut forward, &tokenizer, req);
                // Ignoramos si el receptor ya no existe (request cancelado)
                let _ = reply.send(result);
            }
            InferenceMsg::Stream { req, token_tx } => {
                do_stream(&mut forward, &tokenizer, req, token_tx);
            }
        }
    }

    tracing::info!("InferenceActor thread terminado (canal cerrado)");
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers síncronos (corren dentro del actor thread)
// ─────────────────────────────────────────────────────────────────────────────

fn do_complete(
    forward: &mut StreamingForward,
    tokenizer: &GGUFTokenizer,
    req: CompletionRequest,
) -> Result<CompletionResponse> {
    let prompt_tokens = tokenizer.encode(&req.prompt, true);
    let prompt_count = prompt_tokens.len();
    let eos = tokenizer.eos_token();

    tracing::info!(
        "complete: prompt_tokens={}, max_new={}, temp={}",
        prompt_count, req.max_tokens, req.temperature
    );

    forward.reset();

    let new_tokens = forward.generate(&prompt_tokens, req.max_tokens, req.temperature, eos)?;
    let completion_count = new_tokens.len();
    let text = tokenizer.decode(&new_tokens);

    Ok(CompletionResponse {
        model: req.model,
        text,
        prompt_tokens: prompt_count,
        completion_tokens: completion_count,
    })
}

fn do_stream(
    forward: &mut StreamingForward,
    tokenizer: &GGUFTokenizer,
    req: CompletionRequest,
    token_tx: mpsc::UnboundedSender<Option<String>>,
) {
    let prompt_tokens = tokenizer.encode(&req.prompt, true);
    let eos = tokenizer.eos_token();

    tracing::info!(
        "stream: prompt_tokens={}, max_new={}, temp={}",
        prompt_tokens.len(), req.max_tokens, req.temperature
    );

    forward.reset();

    let result = forward.generate_streaming(
        &prompt_tokens,
        req.max_tokens,
        req.temperature,
        eos,
        |token_id| {
            let text = tokenizer.decode(&[token_id]);
            // Si el receptor desconectó (cliente cerró la conexión) paramos
            if token_tx.send(Some(text)).is_err() {
                tracing::debug!("stream: cliente desconectado, abortando");
            }
        },
    );

    if let Err(e) = result {
        tracing::error!("stream: error de inferencia: {}", e);
    }

    // Señal de fin (None = stream cerrado)
    let _ = token_tx.send(None);
}

// ─────────────────────────────────────────────────────────────────────────────
// Status
// ─────────────────────────────────────────────────────────────────────────────

/// Estado del backend en tiempo de ejecución.
#[derive(serde::Serialize)]
pub struct StreamingStatus {
    pub loaded: bool,
    pub model: Option<String>,
    pub vocab_size: Option<usize>,
    pub n_layers: Option<usize>,
}
