use actix_web::{web, HttpResponse, HttpRequest, HttpMessage};
use crate::models::{ChatCompletionRequest};
use crate::providers::{LLMProvider, ProviderError};
use crate::tracking::RequestTracker;
use crate::middleware::auth::ValidatedApiKey;
use log::{info, error};
use std::sync::RwLock;
use futures::StreamExt;

pub async fn chat_completions(
    req: HttpRequest,
    provider: web::Data<dyn LLMProvider>,
    request_tracker: web::Data<RwLock<RequestTracker>>,
    body: web::Json<ChatCompletionRequest>,
) -> HttpResponse {
    let request = body.into_inner();

    let is_streaming = request.stream.unwrap_or(false);

    if is_streaming {
        info!("Streaming request received");

        let api_key = req.extensions()
            .get::<ValidatedApiKey>()
            .map(|k| k.key.clone())
            .unwrap_or_else(|| "unknown".to_string());
            
        match provider.chat_stream(request).await {
            Ok(stream) => {
                let tracker_for_closure = request_tracker.clone(); // Clone the Arc<RwLock<RequestTracker>>

                let stream = stream.map(move |result| {
                    if let Ok(bytes) = &result {
                        
                        let s = String::from_utf8_lossy(bytes);
                        if s.starts_with("data: ") && !s.contains("[DONE]") {
                             let json_str = s.trim_start_matches("data: ").trim();
                             if let Ok(chunk) = serde_json::from_str::<crate::models::ChatCompletionChunk>(json_str) {
                                 if let Some(usage) = chunk.usage {
                                     let prompt_tokens = usage.prompt_tokens as u64;
                                     let completion_tokens = usage.completion_tokens as u64;
                                     let model = &chunk.model;
                                     
                                     if let Ok(mut t) = tracker_for_closure.write() {
                                         t.record_tokens(&api_key, prompt_tokens, completion_tokens, model);
                                          info!("Recorded streaming tokens: {}p + {}c for {}", prompt_tokens, completion_tokens, api_key);
                                     } else {
                                         error!("Failed to acquire write lock on RequestTracker for streaming usage");
                                     }
                                 }
                             }
                        }
                    }
                    result.map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))
                });

                HttpResponse::Ok()
                .content_type("text/event-stream")
                .streaming(stream)
            }
            Err(e) => error_to_response(e),
        }
    } else {
        info!("Non-streaming request received");

        match provider.chat(request).await {
            Ok(response) => {
                // Record token usage
                if let Some(extensions) = req.extensions().get::<ValidatedApiKey>() {
                    let api_key = &extensions.key;
                    let prompt_tokens = response.usage.prompt_tokens as u64;
                    let completion_tokens = response.usage.completion_tokens as u64;
                    let model = response.model.clone();

                    // Acquire write lock and record
                    if let Ok(mut tracker) = request_tracker.write() {
                        tracker.record_tokens(api_key, prompt_tokens, completion_tokens, &model);
                        info!("Recorded {} tokens for key ending in ...{}", 
                            prompt_tokens + completion_tokens, 
                            &api_key[api_key.len().saturating_sub(4)..]);
                    } else {
                        error!("Failed to acquire write lock on RequestTracker");
                    }
                } else {
                    error!("ValidatedApiKey missing from request extensions");
                }

                HttpResponse::Ok().json(response)
            },
            Err(e) => error_to_response(e),
        }
    }
}

fn error_to_response(err: ProviderError) -> HttpResponse {
    match err {
        ProviderError::Network(msg) => {
            HttpResponse::BadGateway().body(format!("Provider unavailable: {}", msg))
        }
        ProviderError::Parse(msg) => {
            HttpResponse::InternalServerError().body(format!("Failed to parse response: {}", msg))
        }
        ProviderError::ProviderError { status, message } => HttpResponse::build(
            actix_web::http::StatusCode::from_u16(status)
                .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR),
        )
        .body(message),
    }
}
