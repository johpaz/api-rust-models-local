use axum::{
    extract::{Multipart, State},
    Json,
};
use crate::state::AppState;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Deserialize)]
pub struct SpeechRequest {
    pub model: String,
    pub input: String,
    pub voice: Option<String>,
    #[serde(default = "default_speech_response_format")]
    pub response_format: String,
    #[serde(default = "default_speech_speed")]
    pub speed: f32,
}

fn default_speech_response_format() -> String {
    "mp3".to_string()
}

fn default_speech_speed() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
pub struct TranscriptionRequest {
    #[serde(skip)]
    pub file_data: Option<Vec<u8>>,
    pub model: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    #[serde(default = "default_response_format")]
    pub response_format: String,
    #[serde(default)]
    pub temperature: f32,
}

fn default_response_format() -> String {
    "json".to_string()
}

#[derive(Debug, Serialize)]
pub struct TranscriptionResponse {
    pub text: String,
    pub language: Option<String>,
    pub duration: Option<f32>,
}

/// Text-to-Speech: Convertir texto a audio
pub async fn create_speech(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<SpeechRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // llama-server doesn't support TTS natively.
    // This would require a separate TTS model (VITS, Bark, etc.)

    tracing::info!("TTS request: {} chars", payload.input.len());

    Err(AppError::NotImplemented(
        "Text-to-Speech requires a dedicated TTS model (VITS, Bark). llama.cpp does not support this natively.".to_string()
    ))
}

/// Speech-to-Text: Transcribir audio a texto
pub async fn create_transcription(
    State(_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<TranscriptionResponse>, AppError> {
    let mut audio_data: Option<Vec<u8>> = None;
    let mut model = String::new();
    let mut language = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                audio_data = Some(field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?.to_vec());
            }
            "model" => {
                model = String::from_utf8_lossy(&field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?).to_string();
            }
            "language" => {
                language = Some(String::from_utf8_lossy(&field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?).to_string());
            }
            _ => {}
        }
    }

    let audio_data = audio_data.ok_or_else(|| AppError::BadRequest("Missing 'file' field".to_string()))?;

    tracing::info!("Audio transcription request: {} bytes, model={}", audio_data.len(), model);

    // llama.cpp supports Whisper GGUF for transcription
    // Send to llama-server's /infill endpoint or similar
    // For now, return placeholder

    Ok(Json(TranscriptionResponse {
        text: "[Audio transcription placeholder - requires Whisper GGUF model]".to_string(),
        language,
        duration: None,
    }))
}
