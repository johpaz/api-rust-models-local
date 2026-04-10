use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    Json,
};
use crate::state::AppState;
use crate::error::AppError;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

// ============================================================================
// Request/Response Types
// ============================================================================

/// Request for single vision analysis
#[derive(Debug, Deserialize)]
pub struct VisionAnalysisRequest {
    /// Base64 encoded image (JPEG or PNG)
    pub image_base64: String,
    /// Model to use (defaults to current model)
    pub model: Option<String>,
    /// Custom prompt (optional)
    pub prompt: Option<String>,
    /// Max tokens for response
    pub max_tokens: Option<usize>,
    /// Temperature
    pub temperature: Option<f32>,
}

/// Request for batch vision analysis (multiple images)
#[derive(Debug, Deserialize)]
pub struct VisionBatchRequest {
    /// Array of base64 encoded images
    pub images: Vec<VisionImageRequest>,
    /// Model to use
    pub model: Option<String>,
    /// Custom prompt template
    pub prompt: Option<String>,
    /// Max tokens per image
    pub max_tokens: Option<usize>,
    /// Temperature
    pub temperature: Option<f32>,
    /// Process sequentially or in parallel (default: false = parallel)
    #[serde(default)]
    pub parallel: bool,
}

/// Single image in batch request
#[derive(Debug, Deserialize)]
pub struct VisionImageRequest {
    /// Base64 encoded image
    pub image_base64: String,
    /// Optional custom prompt for this specific image
    pub prompt: Option<String>,
    /// Optional image ID for tracking
    pub id: Option<String>,
}

/// Request for WebSocket streaming
#[derive(Debug, Deserialize)]
pub struct VisionWebSocketConfig {
    /// Model to use
    pub model: Option<String>,
    /// Prompt template
    pub prompt: Option<String>,
    /// Max tokens
    pub max_tokens: Option<usize>,
    /// Temperature
    pub temperature: Option<f32>,
}

/// Response for single vision analysis
#[derive(Debug, Serialize)]
pub struct VisionAnalysisResponse {
    pub id: String,
    pub model: String,
    pub content: String,
    pub created: i64,
    pub processing_time_ms: u64,
}

/// Response for batch analysis
#[derive(Debug, Serialize)]
pub struct VisionBatchResponse {
    pub id: String,
    pub model: String,
    pub total_images: usize,
    pub successful: usize,
    pub failed: usize,
    pub results: Vec<VisionImageResult>,
    pub total_processing_time_ms: u64,
}

/// Result for single image in batch
#[derive(Debug, Serialize)]
pub struct VisionImageResult {
    pub id: Option<String>,
    pub index: usize,
    pub success: bool,
    pub content: Option<String>,
    pub error: Option<String>,
    pub processing_time_ms: u64,
}

/// WebSocket message from server to client
#[derive(Debug, Serialize)]
pub struct VisionWSMessage {
    #[serde(rename = "type")]
    pub message_type: String, // "analysis", "error", "status", "ready"
    pub frame_number: Option<u64>,
    pub model: Option<String>,
    pub content: Option<String>,
    pub error: Option<String>,
    pub timestamp: String,
    pub processing_time_ms: Option<u64>,
}

// ============================================================================
// Endpoints
// ============================================================================

/// Analyze a single image (non-streaming JSON response)
pub async fn analyze_image(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<VisionAnalysisRequest>,
) -> Result<Json<VisionAnalysisResponse>, AppError> {
    let start_time = std::time::Instant::now();
    
    if payload.image_base64.is_empty() {
        return Err(AppError::InvalidRequest("image_base64 is required".to_string()));
    }
    
    let prompt = payload.prompt
        .as_deref()
        .unwrap_or("Describe esta imagen en detalle con el mayor nivel de detalle posible.")
        .to_string();
    
    let formatted_prompt = format!("{} <|image|>", prompt);
    
    let model_name = match payload.model {
        Some(m) => m,
        None => state.engine.get_model_name().await,
    };
    
    let max_tokens = payload.max_tokens.unwrap_or(1024);
    let temperature = payload.temperature.unwrap_or(0.7);
    
    tracing::info!(
        "Vision analysis: model={}, prompt_len={}, max_tokens={}",
        model_name,
        formatted_prompt.len(),
        max_tokens
    );
    
    let mut rx = state.engine.generate_stream_with_image(
        formatted_prompt,
        temperature,
        max_tokens,
        vec![],
        Some(model_name.clone()),
        Some(payload.image_base64.clone()),
    ).await?;
    
    let mut full_content = String::new();
    while let Some(token) = rx.recv().await {
        full_content.push_str(&token);
    }
    
    let processing_time_ms = start_time.elapsed().as_millis() as u64;
    
    tracing::info!(
        "Vision completed: model={}, tokens={}, time={}ms",
        model_name,
        full_content.len(),
        processing_time_ms
    );
    
    Ok(Json(VisionAnalysisResponse {
        id: Uuid::new_v4().to_string(),
        model: model_name,
        content: full_content,
        created: Utc::now().timestamp(),
        processing_time_ms,
    }))
}

/// Analyze multiple images in batch (parallel or sequential)
pub async fn analyze_batch(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<VisionBatchRequest>,
) -> Result<Json<VisionBatchResponse>, AppError> {
    let start_time = std::time::Instant::now();
    
    if payload.images.is_empty() {
        return Err(AppError::InvalidRequest("images array is required".to_string()));
    }
    
    if payload.images.len() > 20 {
        return Err(AppError::InvalidRequest("Maximum 20 images per batch".to_string()));
    }
    
    let model_name = match payload.model {
        Some(m) => m,
        None => state.engine.get_model_name().await,
    };
    
    let max_tokens = payload.max_tokens.unwrap_or(1024);
    let temperature = payload.temperature.unwrap_or(0.7);
    
    tracing::info!(
        "Vision batch: model={}, images={}, parallel={}, max_tokens={}",
        model_name,
        payload.images.len(),
        payload.parallel,
        max_tokens
    );
    
    let mut results = Vec::new();
    let mut successful = 0;
    let mut failed = 0;
    
    if payload.parallel {
        // Process in parallel
        let futures: Vec<_> = payload.images.iter().enumerate().map(|(index, img_req)| {
            let state = state.clone();
            let model = model_name.clone();
            let max_tokens = max_tokens;
            let temperature = temperature;
            let prompt = payload.prompt.clone().or_else(|| img_req.prompt.clone())
                .unwrap_or_else(|| "Describe esta imagen en detalle.".to_string());
            
            async move {
                let img_start = std::time::Instant::now();
                let formatted_prompt = format!("{} <|image|>", prompt);
                
                match state.engine.generate_stream_with_image(
                    formatted_prompt,
                    temperature,
                    max_tokens,
                    vec![],
                    Some(model.clone()),
                    Some(img_req.image_base64.clone()),
                ).await {
                    Ok(mut rx) => {
                        let mut content = String::new();
                        while let Some(token) = rx.recv().await {
                            content.push_str(&token);
                        }
                        
                        VisionImageResult {
                            id: img_req.id.clone(),
                            index,
                            success: true,
                            content: Some(content),
                            error: None,
                            processing_time_ms: img_start.elapsed().as_millis() as u64,
                        }
                    }
                    Err(e) => VisionImageResult {
                        id: img_req.id.clone(),
                        index,
                        success: false,
                        content: None,
                        error: Some(e.to_string()),
                        processing_time_ms: img_start.elapsed().as_millis() as u64,
                    }
                }
            }
        }).collect();
        
        // Wait for all futures
        results = futures_util::future::join_all(futures).await;
    } else {
        // Process sequentially
        for (index, img_req) in payload.images.iter().enumerate() {
            let img_start = std::time::Instant::now();
            let prompt = payload.prompt.clone().or_else(|| img_req.prompt.clone())
                .unwrap_or_else(|| "Describe esta imagen en detalle.".to_string());
            
            let formatted_prompt = format!("{} <|image|>", prompt);
            
            match state.engine.generate_stream_with_image(
                formatted_prompt,
                temperature,
                max_tokens,
                vec![],
                Some(model_name.clone()),
                Some(img_req.image_base64.clone()),
            ).await {
                Ok(mut rx) => {
                    let mut content = String::new();
                    while let Some(token) = rx.recv().await {
                        content.push_str(&token);
                    }
                    
                    successful += 1;
                    results.push(VisionImageResult {
                        id: img_req.id.clone(),
                        index,
                        success: true,
                        content: Some(content),
                        error: None,
                        processing_time_ms: img_start.elapsed().as_millis() as u64,
                    });
                }
                Err(e) => {
                    failed += 1;
                    results.push(VisionImageResult {
                        id: img_req.id.clone(),
                        index,
                        success: false,
                        content: None,
                        error: Some(e.to_string()),
                        processing_time_ms: img_start.elapsed().as_millis() as u64,
                    });
                }
            }
        }
    }
    
    if payload.parallel {
        successful = results.iter().filter(|r| r.success).count();
        failed = results.iter().filter(|r| !r.success).count();
    }
    
    let total_processing_time_ms = start_time.elapsed().as_millis() as u64;
    
    tracing::info!(
        "Vision batch completed: total={}, success={}, failed={}, time={}ms",
        payload.images.len(),
        successful,
        failed,
        total_processing_time_ms
    );
    
    Ok(Json(VisionBatchResponse {
        id: Uuid::new_v4().to_string(),
        model: model_name,
        total_images: payload.images.len(),
        successful,
        failed,
        results,
        total_processing_time_ms,
    }))
}

/// WebSocket endpoint for real-time continuous vision streaming
pub async fn vision_stream_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_vision_ws(socket, state))
}

async fn handle_vision_ws(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    
    // Send ready message
    let ready_msg = VisionWSMessage {
        message_type: "ready".to_string(),
        frame_number: None,
        model: None,
        content: None,
        error: None,
        timestamp: Utc::now().to_rfc3339(),
        processing_time_ms: None,
    };
    
    if let Ok(json) = serde_json::to_string(&ready_msg) {
        let _ = ws_sender.send(Message::Text(json.into())).await;
    }
    
    let mut frame_counter: u64 = 0;
    let mut default_model = state.engine.get_model_name().await;
    let mut default_prompt = "Describe esta imagen en detalle.".to_string();
    
    while let Some(Ok(msg)) = ws_receiver.next().await {
        match msg {
            Message::Text(text) => {
                // Parse incoming message
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    // Check if it's a config message
                    if json.get("type").and_then(|v| v.as_str()) == Some("config") {
                        if let Some(model) = json.get("model").and_then(|v| v.as_str()) {
                            default_model = model.to_string();
                        }
                        if let Some(prompt) = json.get("prompt").and_then(|v| v.as_str()) {
                            default_prompt = prompt.to_string();
                        }
                        
                        // Send config ack
                        let ack = VisionWSMessage {
                            message_type: "config_ack".to_string(),
                            frame_number: None,
                            model: Some(default_model.clone()),
                            content: None,
                            error: None,
                            timestamp: Utc::now().to_rfc3339(),
                            processing_time_ms: None,
                        };
                        
                        if let Ok(ack_json) = serde_json::to_string(&ack) {
                            let _ = ws_sender.send(Message::Text(ack_json.into())).await;
                        }
                        continue;
                    }
                    
                    // Process image
                    if let Some(image_base64) = json.get("image_base64").and_then(|v| v.as_str()) {
                        frame_counter += 1;
                        let current_frame = frame_counter;
                        
                        let model = json.get("model")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&default_model)
                            .to_string();
                        
                        let prompt = json.get("prompt")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&default_prompt)
                            .to_string();
                        
                        let max_tokens = json.get("max_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(1024) as usize;
                        
                        let temperature = json.get("temperature")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.7) as f32;
                        
                        let start_time = std::time::Instant::now();
                        let formatted_prompt = format!("{} <|image|>", prompt);
                        
                        // Send processing status
                        let processing_msg = VisionWSMessage {
                            message_type: "processing".to_string(),
                            frame_number: Some(current_frame),
                            model: Some(model.clone()),
                            content: None,
                            error: None,
                            timestamp: Utc::now().to_rfc3339(),
                            processing_time_ms: None,
                        };
                        
                        if let Ok(proc_json) = serde_json::to_string(&processing_msg) {
                            let _ = ws_sender.send(Message::Text(proc_json.into())).await;
                        }
                        
                        // Analyze image
                        match state.engine.generate_stream_with_image(
                            formatted_prompt,
                            temperature,
                            max_tokens,
                            vec![],
                            Some(model.clone()),
                            Some(image_base64.to_string()),
                        ).await {
                            Ok(mut rx) => {
                                let mut content = String::new();
                                while let Some(token) = rx.recv().await {
                                    content.push_str(&token);
                                    
                                    // Stream partial content
                                    let partial_msg = VisionWSMessage {
                                        message_type: "partial".to_string(),
                                        frame_number: Some(current_frame),
                                        model: Some(model.clone()),
                                        content: Some(token),
                                        error: None,
                                        timestamp: Utc::now().to_rfc3339(),
                                        processing_time_ms: None,
                                    };
                                    
                                    if let Ok(partial_json) = serde_json::to_string(&partial_msg) {
                                        let _ = ws_sender.send(Message::Text(partial_json.into())).await;
                                    }
                                }
                                
                                // Send complete message
                                let complete_msg = VisionWSMessage {
                                    message_type: "complete".to_string(),
                                    frame_number: Some(current_frame),
                                    model: Some(model.clone()),
                                    content: Some(content),
                                    error: None,
                                    timestamp: Utc::now().to_rfc3339(),
                                    processing_time_ms: Some(start_time.elapsed().as_millis() as u64),
                                };
                                
                                if let Ok(complete_json) = serde_json::to_string(&complete_msg) {
                                    let _ = ws_sender.send(Message::Text(complete_json.into())).await;
                                }
                            }
                            Err(e) => {
                                let error_msg = VisionWSMessage {
                                    message_type: "error".to_string(),
                                    frame_number: Some(current_frame),
                                    model: Some(model),
                                    content: None,
                                    error: Some(e.to_string()),
                                    timestamp: Utc::now().to_rfc3339(),
                                    processing_time_ms: None,
                                };
                                
                                if let Ok(error_json) = serde_json::to_string(&error_msg) {
                                    let _ = ws_sender.send(Message::Text(error_json.into())).await;
                                }
                            }
                        }
                    }
                }
            }
            Message::Binary(data) => {
                // Treat binary data as base64-encoded image
                frame_counter += 1;
                let current_frame = frame_counter;
                
                let image_base64 = BASE64.encode(&data);
                let formatted_prompt = format!("{} <|image|>", default_prompt);
                let start_time = std::time::Instant::now();
                
                // Send processing status
                let processing_msg = VisionWSMessage {
                    message_type: "processing".to_string(),
                    frame_number: Some(current_frame),
                    model: Some(default_model.clone()),
                    content: None,
                    error: None,
                    timestamp: Utc::now().to_rfc3339(),
                    processing_time_ms: None,
                };
                
                if let Ok(proc_json) = serde_json::to_string(&processing_msg) {
                    let _ = ws_sender.send(Message::Text(proc_json.into())).await;
                }
                
                match state.engine.generate_stream_with_image(
                    formatted_prompt,
                    0.7,
                    1024,
                    vec![],
                    Some(default_model.clone()),
                    Some(image_base64),
                ).await {
                    Ok(mut rx) => {
                        let mut content = String::new();
                        while let Some(token) = rx.recv().await {
                            content.push_str(&token);
                        }
                        
                        let complete_msg = VisionWSMessage {
                            message_type: "complete".to_string(),
                            frame_number: Some(current_frame),
                            model: Some(default_model.clone()),
                            content: Some(content),
                            error: None,
                            timestamp: Utc::now().to_rfc3339(),
                            processing_time_ms: Some(start_time.elapsed().as_millis() as u64),
                        };
                        
                        if let Ok(complete_json) = serde_json::to_string(&complete_msg) {
                            let _ = ws_sender.send(Message::Text(complete_json.into())).await;
                        }
                    }
                    Err(e) => {
                        let error_msg = VisionWSMessage {
                            message_type: "error".to_string(),
                            frame_number: Some(current_frame),
                            model: Some(default_model.clone()),
                            content: None,
                            error: Some(e.to_string()),
                            timestamp: Utc::now().to_rfc3339(),
                            processing_time_ms: None,
                        };
                        
                        if let Ok(error_json) = serde_json::to_string(&error_msg) {
                            let _ = ws_sender.send(Message::Text(error_json.into())).await;
                        }
                    }
                }
            }
            Message::Close(_) => {
                tracing::info!("WebSocket connection closed");
                break;
            }
            _ => {}
        }
    }
}
