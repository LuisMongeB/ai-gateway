use actix_web::{web, HttpResponse};
use crate::models::{ChatCompletionRequest};
use crate::providers::{LLMProvider, ProviderError};
use log::info;
use std::sync::Arc;


pub async fn chat_completions(
    provider: web::Data<Arc<dyn LLMProvider>>,
    body: web::Json<ChatCompletionRequest>,
) -> HttpResponse {
    let request = body.into_inner();

    let is_streaming = request.stream.unwrap_or(false);

    if is_streaming {
        info!("Streaming request received");

        match provider.chat_stream(request).await {
            Ok(stream) => {
                HttpResponse::Ok()
                    .content_type("text/event-stream")
                    .streaming(stream)
            }
            Err(e) => error_to_response(e),
        }
    } else {
        info!("Non-streaming request received");

        match provider.chat(request).await {
            Ok(response) => HttpResponse::Ok().json(response),
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
        ProviderError::ProviderError { status, message } => {
            HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR))
                .body(message)
        }
    }
}
