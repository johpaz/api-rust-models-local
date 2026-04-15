use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;
use axum::response::sse::{Event, KeepAlive, Sse};

use crate::engine::CompletionRequest;

use super::AppState;

// ─── Tipos ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct OaiCompletionRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

#[derive(Deserialize)]
pub struct OaiChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct OaiCompletionResponse {
    id: String,
    object: String,
    model: String,
    choices: Vec<OaiChoice>,
    usage: OaiUsage,
}

#[derive(Serialize)]
struct OaiChoice {
    text: String,
    index: usize,
    finish_reason: String,
}

#[derive(Serialize)]
struct OaiChatResponse {
    id: String,
    object: String,
    model: String,
    choices: Vec<ChatChoice>,
    usage: OaiUsage,
}

#[derive(Serialize)]
struct ChatChoice {
    message: ChatMessage,
    index: usize,
    finish_reason: String,
}

#[derive(Serialize)]
struct OaiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

fn default_max_tokens() -> usize { 256 }
fn default_temperature() -> f32 { 0.7 }

fn ts_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", t)
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// POST /v1/completions — completar texto (modo legacy)
pub async fn oai_completions(
    state: State<Arc<AppState>>,
    Json(payload): Json<OaiCompletionRequest>,
) -> Json<serde_json::Value> {
    let g = state.actor.read().await;
    let actor = match &*g {
        Some(a) => a,
        None => return Json(serde_json::json!({
            "error": { "message": "Actor no cargado. POST /api/stream/load primero.", "type": "backend_unavailable" }
        })),
    };

    let req = CompletionRequest {
        model: payload.model,
        prompt: payload.prompt,
        max_tokens: payload.max_tokens,
        temperature: payload.temperature,
    };

    match actor.complete(req).await {
        Ok(r) => Json(serde_json::json!(OaiCompletionResponse {
            id: format!("cmpl-{}", ts_id()),
            object: "text_completion".to_string(),
            model: r.model,
            choices: vec![OaiChoice { text: r.text, index: 0, finish_reason: "stop".to_string() }],
            usage: OaiUsage {
                prompt_tokens: r.prompt_tokens,
                completion_tokens: r.completion_tokens,
                total_tokens: r.prompt_tokens + r.completion_tokens,
            },
        })),
        Err(e) => Json(serde_json::json!({
            "error": { "message": format!("{}", e), "type": "inference_error" }
        })),
    }
}

/// POST /v1/chat/completions
/// `stream: false` → JSON batch | `stream: true` → SSE token a token
pub async fn oai_chat_completions(
    state: State<Arc<AppState>>,
    Json(payload): Json<OaiChatRequest>,
) -> axum::response::Response {
    let prompt = payload
        .messages
        .iter()
        .map(|m| format!("<|{}|>\n{}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n<|assistant|>\n";

    let req = CompletionRequest {
        model: payload.model.clone(),
        prompt,
        max_tokens: payload.max_tokens,
        temperature: payload.temperature,
    };

    if payload.stream {
        chat_sse(state, req, payload.model).await
    } else {
        chat_batch(state, req, payload.model).await
    }
}

async fn chat_batch(
    state: State<Arc<AppState>>,
    req: CompletionRequest,
    model_id: String,
) -> axum::response::Response {
    let g = state.actor.read().await;
    let actor = match &*g {
        Some(a) => a,
        None => return Json(serde_json::json!({
            "error": { "message": "Actor no cargado.", "type": "backend_unavailable" }
        }))
        .into_response(),
    };
    match actor.complete(req).await {
        Ok(r) => Json(serde_json::json!(OaiChatResponse {
            id: format!("chatcmpl-{}", ts_id()),
            object: "chat.completion".to_string(),
            model: model_id,
            choices: vec![ChatChoice {
                message: ChatMessage { role: "assistant".to_string(), content: r.text },
                index: 0,
                finish_reason: "stop".to_string(),
            }],
            usage: OaiUsage {
                prompt_tokens: r.prompt_tokens,
                completion_tokens: r.completion_tokens,
                total_tokens: r.prompt_tokens + r.completion_tokens,
            },
        }))
        .into_response(),
        Err(e) => Json(serde_json::json!({
            "error": { "message": format!("{}", e), "type": "inference_error" }
        }))
        .into_response(),
    }
}

/// SSE: un evento por token. Señal de fin: `data: [DONE]`
async fn chat_sse(
    state: State<Arc<AppState>>,
    req: CompletionRequest,
    model_id: String,
) -> axum::response::Response {
    let g = state.actor.read().await;
    let actor = match &*g {
        Some(a) => a,
        None => return Json(serde_json::json!({
            "error": { "message": "Actor no cargado.", "type": "backend_unavailable" }
        }))
        .into_response(),
    };

    let rx = match actor.stream(req).await {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({
            "error": { "message": format!("{}", e), "type": "inference_error" }
        }))
        .into_response(),
    };

    drop(g); // liberar read lock antes de streamear

    let id = format!("chatcmpl-{}", ts_id());
    let mid = model_id.clone();

    let stream = UnboundedReceiverStream::new(rx).map(move |chunk| {
        let ev = match chunk {
            Some(text) => Event::default().data(
                serde_json::json!({
                    "id": id,
                    "object": "chat.completion.chunk",
                    "model": mid,
                    "choices": [{ "delta": { "content": text }, "index": 0, "finish_reason": null }]
                })
                .to_string(),
            ),
            None => Event::default().data("[DONE]"),
        };
        Ok::<Event, Infallible>(ev)
    });

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}
