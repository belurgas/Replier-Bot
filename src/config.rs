use std::{path::Path, process::exit};

use tokio::fs;
use serde::{Deserialize, Serialize};

use crate::log_info;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub main_config: MainConfig,
    pub bot_settings: BotSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MainConfig {
    pub app_id: i32,
    pub api_hash: String,
    pub app_title: String,
    pub app_shortname: String,
    pub session_file_name: String,
    // pub mistral_token: String,
    pub bot_token: Option<String>,
    pub users: Vec<User>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct User {
    user_id: i64,
    username: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BotSettings {
    pub target_channel: String,
    pub source_channels: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self { main_config: MainConfig {
            session_file_name: "session".to_string(),
            bot_token: Some("token for your own telegram bot @BotFather".to_string()),
            ..Default::default()
        }, bot_settings: Default::default() }
    }
}

impl Config {
    pub async fn load_config() -> Result<Self> {
        let path = "config.json";

        if !Path::new(path).exists() {
            // Создаём default конфиг
            let default_config = Config::default();

            let json_str = serde_json::to_string_pretty(&default_config)?;

            fs::write(path, json_str).await?;
            log_info!("Создан новый конфиг-файл. Замените соответсвующие поля на ваши");
            exit(0);
        }

        // Если файл существует
        let config_str = fs::read_to_string(path).await.expect("Не удалось загрузить конфиг, проверте наличие config.json");
        let config = serde_json::from_str(&config_str).expect("Не удалость спарсить конфиг");

        Ok(config)
    }
}

