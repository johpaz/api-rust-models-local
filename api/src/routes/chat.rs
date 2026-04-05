use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use crate::state::AppState;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_stream::StreamExt;
use futures_util::stream::Stream;
use std::convert::Infallible;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<usize>,
    pub stream: Option<bool>,
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: ChatUsage,
}

#[derive(Debug, Serialize)]
pub struct ChatChoice {
    pub index: usize,
    pub message: ChatMessageResponse,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct ChatMessageResponse {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChunkChoice>,
}

#[derive(Debug, Serialize)]
pub struct ChatChunkChoice {
    pub index: usize,
    pub delta: ChatChunkDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatChunkDelta {
    pub role: Option<String>,
    pub content: Option<String>,
}

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let prompt = payload.messages.last().map(|m| m.content.clone()).unwrap_or_default();
    
    // Validation
    if prompt.len() > 10000 {
        return Err(AppError::InvalidRequest("Prompt too long".to_string()));
    }
    
    let temperature = payload.temperature.unwrap_or(state.config.default_temperature);
    if temperature < 0.0 || temperature > 2.0 {
        return Err(AppError::InvalidRequest("Temperature must be between 0 and 2".to_string()));
    }
    
    let max_tokens = payload.max_tokens.unwrap_or(1024);
    if max_tokens > 4096 {
        return Err(AppError::InvalidRequest("Max tokens cannot exceed 4096".to_string()));
    }
    
    let stop = payload.stop.unwrap_or_default();

    let mut rx = state.engine.generate_stream(prompt, temperature, max_tokens, stop).await?;
    let id = Uuid::new_v4().to_string();
    let model_name = state.engine.get_model_name();

    let stream = async_stream::stream! {
        yield Ok(Event::default().data(serde_json::to_string(&ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: Utc::now().timestamp(),
            model: model_name.clone(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatChunkDelta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
        }).unwrap()));

        while let Some(token) = rx.recv().await {
            yield Ok(Event::default().data(serde_json::to_string(&ChatCompletionChunk {
                id: id.clone(),
                object: "chat.completion.chunk".to_string(),
                created: Utc::now().timestamp(),
                model: model_name.clone(),
                choices: vec![ChatChunkChoice {
                    index: 0,
                    delta: ChatChunkDelta {
                        role: None,
                        content: Some(token),
                    },
                    finish_reason: None,
                }],
            }).unwrap()));
        }

        yield Ok(Event::default().data(serde_json::to_string(&ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: Utc::now().timestamp(),
            model: model_name.clone(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatChunkDelta {
                    role: None,
                    content: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
        }).unwrap()));

        yield Ok(Event::default().data("[DONE]"));
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
