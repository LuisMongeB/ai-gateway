mod handlers;
mod middleware;
mod models;
mod providers;
mod tracking;

use crate::{
    middleware::{AuthMiddleware, RateLimitMiddleware, RateLimiter, TrackingMiddleware},
    tracking::RequestTracker,
};
use handlers::{chat_completions, get_stats};
use providers::{ollama::OllamaProvider, openai::OpenAIProvider, FallbackProvider, LLMProvider};

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

    let ollama_provider = Arc::new(OllamaProvider::new(
        env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string()),
    ));

    let openai_provider =
        if let (Ok(key), Ok(url)) = (env::var("OPENAI_API_KEY"), env::var("OPENAI_BASE_URL")) {
            Some(Arc::new(OpenAIProvider::new(url, key)))
        } else {
            None
        };

    // Default strategy: Try Ollama, allow fallback to OpenAI if configured
    let provider: Arc<dyn LLMProvider> = if let Some(secondary) = openai_provider {
        // If we have both, use FallbackProvider
        Arc::new(FallbackProvider::new(ollama_provider, secondary))
    } else {
        // If only Ollama, just use Ollama
        ollama_provider
    };

    info!("AI Provider configured. Fallback strategy active if OpenAI keys present.");

    let request_tracker = match RequestTracker::load_from_file("stats.json") {
        Ok(tracker) => {
            info!("Loaded existing request stats from stats.json");
            Arc::new(RwLock::new(tracker))
        }
        Err(_) => {
            info!("No existing stats found, starting fresh");
            Arc::new(RwLock::new(RequestTracker::new()))
        }
    };

    let tracker_for_server = request_tracker.clone();
    let api_keys_for_server = api_keys.clone();
    let admin_keys_for_server = admin_keys.clone();
    let provider_for_server = provider.clone();

    let rate_limiter = Arc::new(RateLimiter::new(60)); // 60 RPM
    let rate_limiter_for_server = rate_limiter.clone();

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(TrackingMiddleware::new(tracker_for_server.clone()))
            // AuthMiddleware must run BEFORE RateLimitMiddleware to set the key.
            // Actix middlewares run in REVERSE definition order.
            // So definition: wrap(RateLimit) -> wrap(Auth)
            // Execution: Auth -> RateLimit -> Handler
            .wrap(RateLimitMiddleware::new(rate_limiter_for_server.clone()))
            .wrap(AuthMiddleware::new(
                api_keys_for_server.clone(),
                admin_keys_for_server.clone(),
            ))
            // We need to wrap in web::Data here explicitly or inside the App?
            // In the previous code: `app_data(web::Data::new(request_tracker.clone()))`
            // `tracker_for_server` is `Arc<RwLock<...>>`. `web::Data` wants to wrap it.
            .app_data(web::Data::from(tracker_for_server.clone()))
            .app_data(web::Data::from(provider_for_server.clone()))
            .service(
                web::scope("/v1")
                    .route("/health", web::get().to(health))
                    .route("/chat/completions", web::post().to(chat_completions))
                    .route("/stats", web::get().to(get_stats)),
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run();

    info!("Server running at http://127.0.0.1:8080");

    server.await?;

    info!("Server shutting down, saving stats...");
    // Save the request tracker before exiting
    if let Err(e) = request_tracker.read().unwrap().save_to_file("stats.json") {
        eprintln!("Failed to save request stats: {}", e);
    } else {
        info!("Request stats saved to stats.json");
    }

    Ok(())
}
