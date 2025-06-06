use tokio::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub main_config: MainConfig,
    pub bot_settings: BotSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainConfig {
    pub app_id: i32,
    pub api_hash: String,
    pub app_title: String,
    pub app_shortname: String,
    pub bot_token: Option<String>,
    pub users: Vec<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    user_id: i64,
    username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSettings {
    pub target_channel: String,
    pub source_channels: Vec<String>,
}

impl Config {
    pub async fn load_config() -> Self {
        let path = "config.json";
        let config_str = fs::read_to_string(path).await.expect("Не удалось загрузить конфиг, проверте наличие config.json");
        let config = serde_json::from_str(&config_str).expect("Не удалость спарсить конфиг");

        config
    }
}