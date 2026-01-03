mod models;
mod handlers;
mod providers;

use handlers::chat_completions;

use actix_web::{web, App, HttpServer, HttpResponse};
use log::info;

async fn health() -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    info!("Starting AI Gateway on port 8080");

    let client = web::Data::new(reqwest::Client::new());

    HttpServer::new(move || {
        App::new()
            .app_data(client.clone())
            .route("/health", web::get().to(health))
            .route("/v1/chat/completions", web::post().to(chat_completions))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
