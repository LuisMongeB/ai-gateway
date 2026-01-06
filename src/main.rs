mod handlers;
mod middleware;
mod models;
mod providers;
mod tracking;

use crate::{
    middleware::{AuthMiddleware, TrackingMiddleware},
    tracking::RequestTracker,
};
use handlers::{chat_completions, get_stats};
use providers::{ollama::OllamaProvider, openai::OpenAIProvider, LLMProvider};

use actix_web::{middleware::Logger, web, App, HttpResponse, HttpServer};
use dotenv::dotenv;
use std::env;
use std::sync::{Arc, RwLock};
use tracing::info;

async fn health() -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "text".to_string());
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if log_format.to_lowercase() == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    dotenv().ok();

    let raw_keys = env::var("GATEWAY_API_KEYS").unwrap_or_else(|_| "secret-key".to_string());
    let raw_admin_keys = env::var("ADMIN_API_KEYS").unwrap_or_else(|_| String::new());

    // 2. Parse into a list (Split by comma!)
    let api_keys: Vec<String> = raw_keys
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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
            let base_url = env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string());
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
            .wrap(TrackingMiddleware::new(request_tracker.clone()))
            .wrap(AuthMiddleware::new(api_keys.clone(), admin_keys.clone()))
            .app_data(provider_data.clone())
            .app_data(request_data.clone())
            .route("/health", web::get().to(health))
            .route("/v1/chat/completions", web::post().to(chat_completions))
            .route("/v1/stats", web::get().to(get_stats))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
