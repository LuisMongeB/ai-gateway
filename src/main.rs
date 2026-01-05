mod models;
mod handlers;
mod providers;
mod middleware;
mod tracking;

use handlers::chat_completions;
use providers::{openai::OpenAIProvider, ollama::OllamaProvider, LLMProvider};
use crate::{middleware::{AuthMiddleware, TrackingMiddleware}, tracking::RequestTracker};


use actix_web::{web, App, HttpServer, HttpResponse, middleware::Logger};
use log::info;
use std::sync::{Arc, RwLock};
use dotenv::dotenv;
use std::env;


async fn health() -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    dotenv().ok();

    let raw_keys = env::var("GATEWAY_API_KEYS").unwrap_or_else(|_| "secret-key".to_string());
    let raw_admin_keys = env::var("ADMIN_API_KEYS").unwrap_or_else(|_| String::new());

    // 2. Parse into a list (Split by comma!)
    let api_keys: Vec<String> = raw_keys
        .split(',')                           // Split at every comma
        .map(|s| s.trim().to_string())        // Remove spaces
        .filter(|s| !s.is_empty())            // Ignore empty strings
        .collect();

    // Parse admin keys
    let admin_keys: Vec<String> = raw_admin_keys
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    

    info!("Loaded {} API keys.", api_keys.len());
    info!("Loaded {} admin API keys.", admin_keys.len());

    let provider_type = env::var("AI_PROVIDER").unwrap_or("ollama".to_string());

    info!("Selected provider: {}", provider_type);

    let provider: Arc<dyn LLMProvider> = match provider_type.as_str() {
        "openai" => {
            let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
            let base_url = env::var("OPENAI_BASE_URL").expect("OPENAI_BASE_URL must be set.");
            Arc::new(OpenAIProvider::new(base_url, api_key))
        }
        "ollama" => {
            let base_url = env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
            Arc::new(OllamaProvider::new(base_url))
        }
        _ => panic!("Unknown provider: {}", provider_type),
    };

    let provider_data = web::Data::from(provider);

    info!("Starting AI Gateway at https://localhost:8080");

    let request_tracker = Arc::new(RwLock::new(RequestTracker::new()));
    let request_data = web::Data::from(request_tracker.clone());

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(AuthMiddleware::new(api_keys.clone(), admin_keys.clone()))
            .wrap(TrackingMiddleware::new(request_tracker.clone()))
            .app_data(provider_data.clone())
            .app_data(request_data.clone())
            .route("/health", web::get().to(health))
            .route("/v1/chat/completions", web::post().to(chat_completions))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
