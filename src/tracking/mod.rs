use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::time::SystemTime;

/// Tracks request metrics across all API keys
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RequestTracker {
    stats: HashMap<String, KeyStats>,
}

/// Per-API-key statistics
#[derive(Debug, Serialize, Deserialize)]
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

    pub fn load_from_file(path: &str) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let tracker = serde_json::from_reader(reader)?;
        Ok(tracker)
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }

    /// Record a completed request (called by middleware after response)
    pub fn record_request(&mut self, api_key: &str, latency_ms: u64, is_error: bool) {
        let stats = self
            .stats
            .entry(api_key.to_string())
            .or_insert_with(KeyStats::new);
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
        let stats = self
            .stats
            .entry(api_key.to_string())
            .or_insert_with(KeyStats::new);
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

/// Custom serializer/deserializer for SystemTime as milliseconds since UNIX epoch
mod system_time_as_millis {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, SystemTime};

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

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(SystemTime::UNIX_EPOCH + Duration::from_millis(millis))
    }
}
