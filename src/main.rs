use dotenv::dotenv;

use grammers_client::types::{Channel, Chat, Downloadable, Media};
use grammers_client::{Client, InputMedia, InputMessage, InvocationError, Update};
use serde::Deserialize;
use tokio::time::{interval, sleep};
use std::collections::HashSet;
use std::time::Duration;

use crate::handlers::{generate, MediaGroupHandler};
use crate::logging::logger::setup_logger;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod handlers;
mod config;
mod login;
mod bot;
mod mistral;
mod logging;

#[derive(Debug, Deserialize)]
struct AproveData {
    status: String,
    text: String,
}


async fn join_channels(client: &mut Client, channels: &Vec<Channel>) {
    for username in channels {
        if let Err(e) = client.join_chat(username).await {
                    log_error!("could not subscribe to {}: {:?}", username.title(), e);
                } else {
                    log_info!("Subscribed to {}", username.title());
                }
        sleep(Duration::from_millis(1500)).await;
    }
}

#[derive(Debug, Default)]                        
enum MediaType {
    #[default]
    None,
    Photo,
}

async fn monitor_and_forward(client: &mut Client, target_channel: &str, chated: Vec<Channel>, mistral_token: &str) -> Result<()> {
    let target = match client.resolve_username(target_channel).await? {
        Some(t) => t,
        None => return Err("Target channel not found".into()),
    };

    let group_handler = MediaGroupHandler::new(Duration::from_secs(1)).await;
    let handler_clone = group_handler.clone();
    let client_clone = client.clone();
    let target_clone = target.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            for (_, messages) in handler_clone.get_expired_groups().await {
                if let Err(e) = client_clone.send_album(target_clone.clone(), messages).await {
                    log_error!("Error seending media group: {}", e);
                }
            }
        }
    });


    let mut none_relevant_group: HashSet<i64> = HashSet::new();
    client.sync_update_state();
    log_info!("Sync state");

    loop {
        let upd = client.next_update().await.unwrap();
        match upd {
            Update::NewMessage(msg) if !msg.outgoing() => {
                match msg.chat() {
                    Chat::Channel(ch) => {
                        log_info!("New message");
                        if is_chat_in_list(&ch, &chated) {
                            // Anticopy rules
                            // download only first message
                            if let Some(group_id) = msg.grouped_id() {
                                if none_relevant_group.contains(&group_id) {
                                    continue;
                                }
                            }
                            if !msg.text().is_empty() {
                                let msg_in_cahrs = msg.text().chars().count();                                
                                let gend = generate(msg.text(), mistral_token).await?;
                                if let Some(status_message) = gend.choices.first() {
                                    let message = status_message.message.content.clone();                              
                                    match serde_json::from_str::<AproveData>(&message) {
                                        Ok(jj) => {
                                            if jj.status != "релевантный" {
                                                log_info!("No relevante message");
                                                if let Some(group_id) = msg.grouped_id() {
                                                    none_relevant_group.clear();
                                                    none_relevant_group.insert(group_id);
                                                }
                                                continue;
                                            } else {
                                                log_info!("Relevante message");
                                            }

                                            if msg_in_cahrs > 1000 {
                                                let message = jj.text;
                                                log_info!("The text that should be: {}", message);
                                            }
                                        }
                                        Err(e) => {
                                            log_error!("Json parsing error: {:?}", e);
                                        }
                                    }
                                }
                                log_info!("The tratment was successful.");
                            }
                            if ch.raw.noforwards {
                                if let Some(media) = msg.media() {
                                    let mut path = String::new();
                                    let mut media_type = MediaType::None;

                                    match media.clone() {
                                        Media::Photo(_) => {
                                            path = "./image.jpg".to_string();
                                            media_type = MediaType::Photo
                                        },
                                        Media::Document(doc) => {
                                            if let Some(mime) = doc.mime_type() {
                                                if let Some(mime_format) = mime.split('/').last() {
                                                    path = format!("./doc.{}", mime_format);
                                                }
                                            } else {
                                                log_warn!("None mime type: {}", doc.name());
                                            }
                                        }
                                        _ => {
                                            // Nothin do with another media
                                        }
                                    };

                                    let down = Downloadable::Media(media.clone());
                                    client.download_media(&down, &path).await?;

                                    let upload = client.upload_file(&path).await?;
                                    
                                    if !msg.text().is_empty() {
                                        let text = msg.text();
                                        match media_type {
                                            MediaType::Photo => {
                                                if let Err(e) = client.send_message(target.clone(), InputMessage::text(text).photo(upload)).await {
                                                    match e {
                                                        InvocationError::Rpc(rpc) => {
                                                            log_error!("Error noforward media send: {:?}", rpc);
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                            _ => {                                                                                                           
                                                client.send_message(target.clone(), InputMessage::text(text).document(upload)).await?;
                                            }
                                        }
                                    } else {
                                        if let None = msg.grouped_id() {
                                            // Only message without text
                                            client.send_message(target.clone(), InputMessage::default().document(upload)).await?;   
                                        }
                                    }
                                } else {
                                    // No media, just send text
                                    if !msg.text().is_empty() {
                                        let text = msg.text();
                                        client.send_message(target.clone(), InputMessage::text(text)).await?;
                                    }
                                }
                            } else {
                                if let Some(grouped_id) = msg.grouped_id() {
                                    if let Some(media) = msg.media() {
                                        let caption = msg.text();
                                        let media_data = InputMedia::caption(caption).copy_media(&media);
                                        group_handler.add_media(grouped_id, media_data).await;
                                        
                                    }
                                } else {
                                    msg.forward_to(&target).await?;
                                }
                            }
                        }
                    }
                    Chat::Group(gr) => {
                        log_debug!("Message recv from: {}", gr.title());
                    }
                    d => {
                        log_debug!("Unhandled type of chat: {:?}", d.username());
                    }
                }
            }
            _ => {} 
        }
    }
}

async fn resolve_chnnels(client: &mut Client, channels: Vec<String>) -> Result<Vec<Channel>> {
    let mut channels_chat = Vec::new();

    for name in channels {
        let mut retry_count: u32= 0;
        loop {
            match client.resolve_username(&name).await {
                Ok(Some(chat)) => match chat {
                    Chat::Channel(ch) => {
                        log_info!("Catch resolve: {}", ch.title());
                        channels_chat.push(ch.clone());
                        break;
                    },
                    _ => {
                        // Ничего кроме каналов не добавляем
                    }
                },
                Ok(None) => {    
                    log_warn!("Could not find the chat: {}", name);
                    break;
                },
                Err(e) => {
                    match e {
                        InvocationError::Rpc(rpc) => {
                            if rpc.name == "FLOOD_WAIT" {
                                if let Some(_time_to_wait) = rpc.value {
                                    let time_to_wait = rpc.value.unwrap_or(5); // по умолчанию 5 секунд
                                    log_info!("Catch FLOOD_WAIT, wait for {} seconds after retry...", time_to_wait);
                                    sleep(Duration::from_secs((time_to_wait + 1).into())).await;

                                    retry_count += 1;
                                    if retry_count > 3 {
                                        log_error!("Too mach retries for {}: {:?}", name, rpc);
                                        break;
                                    }
                                    continue;
                                }
                            }
                        }
                        err => {
                            log_error!("Unknown error: {:?}", err);
                            break;
                        }
                    }
                },
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
    Ok(channels_chat)
}

fn is_chat_in_list(chat: &Channel, channels: &Vec<Channel>) -> bool {
    channels.iter().any(|c| 
        c.id() == chat.id() || 
        c.username().map_or(false, |u| u == chat.username().unwrap_or_default())
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    setup_logger().expect("Could not setting up logger");
    let config = crate::config::Config::load_config().await?;
    let init_config = config.clone();
    let api_id = init_config.main_config.app_id;
    let api_hash = init_config.main_config.api_hash;
    let target = init_config.bot_settings.target_channel;
    let session_file_name = init_config.main_config.session_file_name;
    let session_file_name = format!("{}.session", session_file_name);
    let channels = init_config.bot_settings.source_channels;
    // let mistral_token = init_config.main_config.mistral_token;

    let mut client = login::login(api_id, api_hash, &session_file_name).await;
    let me = client.get_me().await.unwrap();
    log_info!(
       "Username: {}", me.username().unwrap_or("No username"));
    let chated = resolve_chnnels(&mut client, channels.clone()).await?;
    log_info!("Channels founded: {}", chated.len());
    join_channels(&mut client, &chated).await;
    if let Err(e) = monitor_and_forward(&mut client, &target, chated, "d").await {
        log_error!("Error from monitor_and_forward: {:?}", e)
    };

    Ok(())
}
