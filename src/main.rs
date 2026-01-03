mod models;
mod handlers;
mod providers;

use handlers::chat_completions;
use providers::ollama::OllamaProvider;
use providers::LLMProvider;

use actix_web::{web, App, HttpServer, HttpResponse};
use log::info;
use std::sync::Arc;


async fn health() -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    info!("Starting AI Gateway on port 8080");

    let provider: Arc<dyn LLMProvider> = Arc::new(
        OllamaProvider::new("http://localhost:11434".to_string())
    );

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(provider.clone()))
            .route("/health", web::get().to(health))
            .route("/v1/chat/completions", web::post().to(chat_completions))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
