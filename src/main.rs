use std::{collections::HashMap, sync::Arc, time::Duration};

use dotenv::dotenv;
use grammers_client::{
    types::{Chat, Message},
    Client, InputMedia, InputMessage, InvocationError,
};

use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, time::sleep};

use crate::handler::generate;

mod login;
mod config;
mod handler;
mod mistral;
mod logging;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;


#[derive(Debug, Deserialize)]
struct AproveData {
    status: String,
    text: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct History {
    messages: HashMap<i64, Vec<i32>>, // chat_id -> Vec<message_id>
}

const HISTORY_FILE: &str = "history.json";

async fn load_history() -> Result<History> {
    if tokio::fs::try_exists(HISTORY_FILE).await? {
        let data = tokio::fs::read_to_string(HISTORY_FILE).await?;
        Ok(serde_json::from_str(&data)?)
    } else {
        Ok(History {
            messages: HashMap::new(),
        })
    }
}

async fn save_history(history: &History) -> Result<()> {
    let data = serde_json::to_string_pretty(history)?;
    tokio::fs::write(HISTORY_FILE, data).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let config = crate::config::Config::load_config().await.unwrap();

    let api_id = config.main_config.app_id;
    let api_hash = config.main_config.api_hash.clone();
    let target_username = config.bot_settings.target_channel.clone();
    let session_file = format!("{}.session", config.main_config.session_file_name);
    let channels = config.bot_settings.source_channels;
    let mistral_token = config.main_config.mistral_token;

    let client = Arc::new(login::login(api_id, api_hash, &session_file).await);
    let me = client.get_me().await?;
    log_info!("Username: {}", me.username().unwrap_or("No username"));

    let target = client.resolve_username(&target_username).await?.unwrap();
    log_info!("Target channel resolved: {:?}", target.name());

    // let source_chats = resolve_channels(&mut client, channels).await?;
    // println!("Resolved {} source channels", source_chats.len());

    let mut input_chats: Vec<Chat> = Vec::new();
    for chat in channels {
        if let Some(ch) = client.resolve_username(&chat).await? {
            input_chats.push(ch.clone());
            log_info!("Not founded: {}", ch.name());
        } else {
            log_info!("Not founded")
        }
        sleep(Duration::from_secs(1)).await;
    }

    process_chat(client, input_chats, target, mistral_token).await?;
    

    loop {
        sleep(Duration::from_secs(3600)).await;
    }
}

async fn process_chat(
    client: Arc<Client>,
    input_chats: Vec<Chat>,
    target_chat: Chat,
    mistral_token: String,
) -> Result<()> {
    let history = Arc::new(Mutex::new(load_history().await?));

    for chat in input_chats {
        let chat_id = chat.id();
        let history_ref = Arc::clone(&history);
        let client = Arc::clone(&client);
        let mistral = mistral_token.clone();
        let target = target_chat.clone();

        tokio::spawn(async move {
            loop {
                // Блокировка только на время работы с историей
                let mut history = history_ref.lock().await;

                let mut messages = client.iter_messages(chat.clone()).limit(10);
                let mut groups: HashMap<i64, Vec<Message>> = HashMap::new();

                // Получаем список сообщений для этого чата
                let chat_messages = history.messages.entry(chat_id).or_insert_with(Vec::new);

                while let Some(message) = messages.next().await.unwrap() {
                    let msg_id = message.id();

                    if chat_messages.contains(&msg_id) {
                        continue;
                    }

                    if let Some(group_id) = message.grouped_id() {
                        chat_messages.push(msg_id);
                        groups.entry(group_id).or_default().push(message);
                    } else {
                        chat_messages.push(msg_id);
                        if let Some(media) = message.media() {
                            let mut has_relevant = false;
                            let mut ai_text = String::new();
                            let gend = generate(message.text(), &mistral).await.unwrap();
                            if let Some(choice) = gend.choices.first() {
                                match serde_json::from_str::<AproveData>(&choice.message.content) {
                                    Ok(data) => {
                                        if data.status == "релевантный" {
                                            has_relevant = true;
                                            ai_text = data.text;
                                        }
                                    }
                                    Err(e) => log_error!("JSON parsing error: {:?}", e),
                                }
                            }
                            if has_relevant {
                                if !message.text().is_empty() {
                                    match client.send_album(target.clone(), vec![InputMedia::caption(ai_text).copy_media(&media)]).await {
                                        Ok(_) => {},
                                        Err(e) => {
                                            match e {                                
                                                InvocationError::Rpc(rpc) => {
                                                    if rpc.name == "FLOOD_WAIT" {
                                                        log_info!("Wait... {:?}", rpc.value);
                                                        sleep(Duration::from_secs(rpc.value.unwrap_or(10).into())).await;
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            } else {
                                continue;
                            }
                        } else {
                            let mut has_relevant = false;
                            let mut ai_text = String::new();
                            let gend = generate(message.text(), &mistral).await.unwrap();
                            if let Some(choice) = gend.choices.first() {
                                match serde_json::from_str::<AproveData>(&choice.message.content) {
                                    Ok(data) => {
                                        if data.status == "релевантный" {
                                            has_relevant = true;
                                            ai_text = data.text;
                                        }
                                    }
                                    Err(e) => log_error!("JSON parsing error: {:?}", e),
                                }
                            }
                            if has_relevant {
                                if !message.text().is_empty() {
                                    match client.send_message(target.clone(), InputMessage::text(ai_text)).await {
                                        Ok(_) => {},
                                        Err(e) => {
                                            match e {                                        
                                                InvocationError::Rpc(rpc) => {
                                                    if rpc.name == "FLOOD_WAIT" {
                                                        log_info!("Wait... {:?}", rpc.value);
                                                        sleep(Duration::from_secs(rpc.value.unwrap_or(10).into())).await;
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    sleep(Duration::from_secs(1)).await;
                }

                drop(history); // освобождаем мьютекс до обработки медиа

                // --- Анализируем и отправляем медиа ---
                for (_group_id, messages_in_group) in groups {
                    let mut media_group: Vec<InputMedia> = Vec::new();
                    let mut has_relevant = false;
                    let mut _ai_text = String::new();

                    for msg in &messages_in_group {
                        if msg.text().is_empty() {
                            continue;
                        }

                        let gend = generate(msg.text(), &mistral).await.unwrap();
                        if let Some(choice) = gend.choices.first() {
                            match serde_json::from_str::<AproveData>(&choice.message.content) {
                                Ok(data) => {
                                    if data.status == "релевантный" {
                                        has_relevant = true;
                                        _ai_text = data.text;
                                    }
                                }
                                Err(e) => log_error!("JSON error parsing: {:?}", e),
                            }
                        }
                    }

                    if has_relevant {
                        for msg in &messages_in_group {
                            if let Some(media) = msg.media() {
                                let caption = msg.text();
                                media_group.push(InputMedia::caption(caption).copy_media(&media));
                            }
                        }
                    }

                    if !media_group.is_empty() {
                        match client.send_album(target.clone(), media_group).await {
                            Ok(_) => log_info!("Success send album"),
                            Err(e) => {
                                if let InvocationError::Rpc(rpc) = &e {
                                    log_info!("Wait... {:?}", rpc.value);
                                    if rpc.name == "FLOOD_WAIT" {
                                        let delay = rpc.value.unwrap_or(10) as u64;
                                        sleep(Duration::from_secs(delay)).await;
                                    }
                                }
                            }
                        }
                    }
                }

                // Сохраняем историю раз в цикл
                let history_to_save = Arc::clone(&history_ref);
                let locked = history_to_save.lock().await;
                if let Err(e) = save_history(&locked).await {
                    log_error!("Error while saving messages history: {}", e);
                }

                sleep(Duration::from_secs(1)).await;
            }
        });
    }

    Ok(())
}