use std::env;

use serde::{Deserialize, Serialize};

/// Logger configuration with .env parse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: String,
    pub file_path: String,
    pub file_size_limit: u64, // в байтах
    pub file_rotation_period: String, // daily, hourly, never
    pub use_json: bool,
    pub enable_sentry: bool,
}

/// LogConfig Default implimentation using env vars
impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: env::var("RUST_LOG").unwrap_or("info".to_string()),
            file_path: env::var("LOG_FILE").unwrap_or("app.log".to_string()),
            file_size_limit: env::var("LOG_FILE_SIZE_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10 * 1024 * 1024), // 10MB
            file_rotation_period: env::var("LOG_ROTATION").unwrap_or("daily".to_string()),
            use_json: env::var("LOG_JSON").is_ok(),
            enable_sentry: env::var("ENABLE_SENTRY").is_ok(),
        }
    }
}