use actix_web::{web, HttpRequest, HttpResponse, HttpMessage};
use crate::middleware::auth::{ApiKeyRole, ValidatedApiKey};
use crate::tracking::RequestTracker;
use serde::Serialize;
use std::sync::{RwLock};
use std::collections::HashMap;


#[derive(serde::Deserialize)]
pub struct StatsQuery {
    pub key: Option<String>,
}

#[derive(Serialize)]
pub struct KeyStatsResponse {
    pub api_key: String,  // Will be masked
    pub request_count: u64,
    pub error_count: u64,
    pub total_latency_ms: u64,
    pub avg_latency_ms: f64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub last_request_timestamp: u64,
    pub models_used: HashMap<String, u64>,
}

pub async fn get_stats(
    req: HttpRequest,
    query: web::Query<StatsQuery>,
    tracker: web::Data<RwLock<RequestTracker>>,
) -> HttpResponse {
    // 1. Extract validated key from request extensions
    let validated_key = req.extensions().get::<ValidatedApiKey>().cloned();

    let Some(validated) = validated_key else {
        return HttpResponse::Unauthorized().body("Missing API key context");
    };

    // 2. Read lock on tracker
    let tracker_guard = tracker.read().unwrap();

    // 3. Branch based on role
    match validated.role {
        ApiKeyRole::Admin => {
            match &query.key {
                // Admin requesting specific key's stats
                Some(target_key) => {
                    match tracker_guard.get_stats(target_key) {
                        Some(stats) => {
                            let response = build_stats_response(target_key, stats);
                            HttpResponse::Ok().json(response)
                        }
                        None => HttpResponse::NotFound().body("No stats for that key"),
                    }
                }
                // Admin requesting all stats
                None => {
                    let all_stats: Vec<KeyStatsResponse> = tracker_guard
                        .get_all_stats()
                        .iter()
                        .map(|(key, stats)| build_stats_response(key, stats))
                        .collect();
                    HttpResponse::Ok().json(all_stats)
                }
            }
        }
        ApiKeyRole::User => {
            // Users can only see their own stats, ignore query.key
            match tracker_guard.get_stats(&validated.key) {
                Some(stats) => {
                    let response = build_stats_response(&validated.key, stats);
                    HttpResponse::Ok().json(response)
                }
                None => {
                    // No stats yet (first request?) — return empty stats
                    HttpResponse::Ok().json(KeyStatsResponse {
                        api_key: mask_key(&validated.key),
                        request_count: 0,
                        error_count: 0,
                        total_latency_ms: 0,
                        avg_latency_ms: 0.0,
                        total_prompt_tokens: 0,
                        total_completion_tokens: 0,
                        last_request_timestamp: 0,
                        models_used: HashMap::new(),
                    })
                }
            }
        }
    }
}

fn build_stats_response(key: &str, stats: &crate::tracking::KeyStats) -> KeyStatsResponse {
    let avg_latency = if stats.request_count > 0 {
        stats.total_latency_ms as f64 / stats.request_count as f64
    } else {
        0.0
    };

    let timestamp = stats
        .last_request_timestamp
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    KeyStatsResponse {
        api_key: mask_key(key),
        request_count: stats.request_count,
        error_count: stats.error_count,
        total_latency_ms: stats.total_latency_ms,
        avg_latency_ms: avg_latency,
        total_prompt_tokens: stats.total_prompt_tokens,
        total_completion_tokens: stats.total_completion_tokens,
        last_request_timestamp: timestamp,
        models_used: stats.models_used.clone(),
    }
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "***".to_string()
    } else {
        let prefix = &key[..4];
        let suffix = &key[key.len() - 4..];
        format!("{}***{}", prefix, suffix)
    }
}