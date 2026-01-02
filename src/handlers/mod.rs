use actix_web::{web, Responder, HttpResponse};

use crate::models::{ChatCompletionRequest, OllamaRequest, OllamaResponse};

pub async fn chat_completions(
    client: web::Data<reqwest::Client>,
    body: web::Json<ChatCompletionRequest>,
) -> impl Responder {
    let request = body.into_inner();

    let ollama_request = OllamaRequest {
        model: request.model,
        messages: request.messages,
        stream: false,
    };

    let response = client
            .post("http://localhost:11434/api/chat")
            .json(&ollama_request)
            .send()
            .await;

    let ollama_response = match response {
        Ok(resp) => resp,
        Err(e) => {
            return HttpResponse::InternalServerError().json(e.to_string())
        }
    };

    match ollama_response.json::<OllamaResponse>().await {
        Ok(ollama_data) => {
            HttpResponse::Ok().json(ollama_data)
        }
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("Failed to parse response: {}", e))
        }
    }
}
