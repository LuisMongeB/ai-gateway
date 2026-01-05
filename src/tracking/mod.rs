use std::collections::HashMap;
use std::time::SystemTime;
use serde::Serialize;

/// Tracks request metrics across all API keys
#[derive(Debug, Default, Serialize)]
pub struct RequestTracker {
    stats: HashMap<String, KeyStats>,
}

/// Per-API-key statistics
#[derive(Debug, Serialize)]
pub struct KeyStats {
    pub request_count: u64,
    pub error_count: u64,
    pub total_latency_ms: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub models_used: HashMap<String, u64>,
    #[serde(with = "system_time_as_millis")]
    pub last_request_timestamp: SystemTime,
}

impl KeyStats {
    fn new() -> Self {
        Self {
            request_count: 0,
            error_count: 0,
            total_latency_ms: 0,
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            models_used: HashMap::new(),
            last_request_timestamp: SystemTime::now(),
        }
    }
}

impl RequestTracker {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    /// Record a completed request (called by middleware after response)
    pub fn record_request(
        &mut self,
        api_key: &str,
        latency_ms: u64,
        is_error: bool,
    ) {
        let stats = self.stats.entry(api_key.to_string()).or_insert_with(KeyStats::new);
        stats.request_count += 1;
        stats.total_latency_ms += latency_ms;
        stats.last_request_timestamp = SystemTime::now();
        if is_error {
            stats.error_count += 1;
        }
    }

    /// Record token usage (called by handler after parsing LLM response)
    pub fn record_tokens(
        &mut self,
        api_key: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
        model: &str,
    ) {
        let stats = self.stats.entry(api_key.to_string()).or_insert_with(KeyStats::new);
        stats.total_prompt_tokens += prompt_tokens;
        stats.total_completion_tokens += completion_tokens;
        *stats.models_used.entry(model.to_string()).or_insert(0) += 1;
    }

    /// Get stats for a specific API key
    pub fn get_stats(&self, api_key: &str) -> Option<&KeyStats> {
        self.stats.get(api_key)
    }

    /// Get all stats (for /stats endpoint)
    pub fn get_all_stats(&self) -> &HashMap<String, KeyStats> {
        &self.stats
    }
}

/// Custom serializer for SystemTime as milliseconds since UNIX epoch
mod system_time_as_millis {
    use serde::{Serializer, Serialize};
    use std::time::SystemTime;

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let millis = time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        millis.serialize(serializer)
    }
}