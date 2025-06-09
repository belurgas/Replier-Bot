use dotenv::dotenv;
use grammers_client::grammers_tl_types::enums::RpcError as rpc;
use grammers_client::types::{Channel, Chat, Downloadable, Media, Message};
use grammers_client::{Client, InputMedia, InputMessage, Update};
use grammers_mtsender::RpcError;
use serde::Deserialize;
use tokio::time::{interval, sleep};
use std::time::Duration;

use crate::handlers::{generate, MediaGroupHandler};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod handlers;
mod config;
mod login;
mod bot;
mod mistral;

#[derive(Debug, Deserialize)]
struct AproveData {
    status: String,
    text: String,
}

async fn join_channels(client: &mut Client, channels: &Vec<Channel>) {
    for username in channels {
        if let Err(e) = client.join_chat(username).await {
                    println!("Не удалось подписаться на {}: {:?}", username.title(), e);
                } else {
                    println!("Подписался на {}", username.title());
                }
        sleep(Duration::from_millis(1500)).await;
    }
}

async fn send_media_group(client: &mut Client, target_channel: Chat, messages: Vec<Message>) -> Result<()> {
    
    let mut album: Vec<InputMedia> = Vec::with_capacity(messages.len());

    // Отправляем совокупные медиа
    for (idx, i) in messages.iter().enumerate() {
        match i.chat() {
            Chat::Channel(ch) => {
                if let Some(media) = i.media() {
                    if let Some(grouped_id) = i.grouped_id() {
                        if ch.raw.noforwards {
                            if !i.text().is_empty() {
                                let path = format!("./image_{}.jpg", idx);
                                let down = Downloadable::Media(media);
                                client.download_media(&down, &path).await?;
                                sleep(Duration::from_millis(500)).await;
                                let uploaded = client.upload_file(path).await?;
                                sleep(Duration::from_millis(500)).await;
                                client.send_message(target_channel, InputMessage::text(i.text()).photo(uploaded)).await?;
                                return Ok(());
                            }
                            return Ok(());
                        } else {    
                            i.forward_to(target_channel.clone()).await?;
                            return Ok(())
                            
                        }
                    } else {
                        if ch.raw.noforwards {
                            if !i.text().is_empty() {
                                let path = format!("./image_{}.jpg", idx);
                                let down = Downloadable::Media(media);
                                client.download_media(&down, &path).await?;
                                sleep(Duration::from_millis(500)).await;
                                let uploaded = client.upload_file(path).await?;
                                sleep(Duration::from_millis(500)).await;
                                client.send_message(target_channel, InputMessage::text(i.text()).photo(uploaded)).await?;
                                return Ok(());
                            }
                            return Ok(());
                        } else {    
                            i.forward_to(target_channel.clone()).await?;
                            return Ok(())
                            
                        }
                    }
                }
            }
            _ => {}
        }
    }

    match album.len() {
        0 => return Ok(()),
        1 => {
            client.send_album(target_channel, vec![album.remove(0)]).await?;
            return Ok(());
        }
        _ => {
            if let Err(e) = client.send_album(target_channel, album).await {
                eprintln!("Ошибка блять: {:?}", e);
            }
            return Ok(());
        }
    };
}

// match client.send_message(target_channel.clone(), InputMessage::text(i.text()).copy_media(&media)).await {
                        //     Ok(okd) => {
                        //         // Получаем сообщение
                        //     },
                        //     Err(e) => {
                        //         match e {
                        //             InvocationError::Rpc(r) => {
                        //                 match r {
                        //                     error => {
                        //                         if error.name == "MEDIA_CAPTION_TOO_LONG".to_string() {
                        //                             println!("Описание слишком большое. Пересылаем");
                        //                             i.forward_to(target_channel.clone()).await?;
                        //                         }
                        //                     }
                        //                 }
                        //             },
                        //             _ => {}
                        //         }
                        //     }
                        // }

async fn monitor_and_forward(client: &mut Client, target_channel: &str, chated: Vec<Channel>, mistral_token: &str) -> Result<()> {
    let target = match client.resolve_username(target_channel).await? {
        Some(t) => t,
        None => return Err("Target channel not found".into()),
    };

    let group_handler = MediaGroupHandler::new(Duration::from_secs(1)).await;
    let handler_clone = group_handler.clone();
    let mut client_clone = client.clone();
    let target_clone = target.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            for (_, messages) in handler_clone.get_expired_groups().await {
                if let Err(e) = send_media_group(&mut client_clone, target_clone.clone(), messages).await {
                    eprintln!("Error seending media group: {}", e);
                }
            }
        }
    });

    loop {
        let upd = client.next_update().await.unwrap();
        match upd {
            Update::NewMessage(msg) => {
                if let Some(username) = msg.chat().username() {
                    if username == target_channel {
                        // Значит получили обновление из таргет канала
                        continue;
                    }
                }

                match msg.chat() {
                    Chat::Channel(ch) => {
                        if is_chat_in_list(&ch, &chated) {
                            let resp = generate(msg.text(), mistral_token).await?;
                            if let Some(status_message) = resp.choices.first() {
                                let message = status_message.message.content.clone();
                                // let aga = message.lines()
                                //     .filter(|line| line.trim().starts_with('{'))
                                //     .collect::<Vec<&str>>()
                                //     .join("");
                                // println!("Aga: {}", aga);
                                let jj: AproveData = serde_json::from_str(&message)?;
                                // println!("Aprove: {:#?}", jj);

                                if jj.status != "релевантный" {
                                    continue;
                                }
                            }

                            if ch.raw.noforwards {
                                println!("Группа с запретом на копирование, где альбом сгрупирован");
                                if let Some(group_id) = msg.grouped_id() {
                                    println!("Группа, собираем!");
                                    group_handler.add_message(group_id, msg.clone()).await;
                                    
                                    
                                } else {
                                    println!("Группа с запретом, где одно медиа");
                                    if let Some(media) = msg.media() {
                                        let mut path = String::new();
                                        match media.clone() {
                                            Media::Document(doc) => {
                                                // Получаем расширение документа
                                                if let Some(mime) = doc.mime_type() {
                                                    println!("doc: {}", mime);
                                                    path = format!("./document.{}", mime.split('/').last().unwrap());
                                                    let down = Downloadable::Media(media.clone());
                                                    client.download_media(&down, &path).await?;
                                                    sleep(Duration::from_millis(500)).await;
                                                    let uploaded = client.upload_file(&path).await?;
                                                    sleep(Duration::from_millis(500)).await;
                                                    client.send_message(target.clone(), InputMessage::text(msg.text()).document(uploaded)).await?;
                                                }
                                            }
                                            Media::Photo(phot) => {
                                                path = "./image.jpg".to_string();
                                                let down = Downloadable::Media(media.clone());
                                                client.download_media(&down, &path).await?;
                                                sleep(Duration::from_millis(500)).await;
                                                let uploaded = client.upload_file(&path).await?;
                                                sleep(Duration::from_millis(500)).await;
                                                client.send_message(target.clone(), InputMessage::text(msg.text()).photo(uploaded)).await?;
                                            },
                                            d => {
                                                println!("Media: {:#?}", d);
                                            }
                                        }
                                    } else {
                                        client.send_message(target.clone(), InputMessage::text(msg.text())).await?;
                                    }
                                    continue;
                                }
                                continue;
                            } else {
                                if let Some(s) = msg.media() {
                                    if let Some(group_id) = msg.grouped_id() {
                                        group_handler.add_message(group_id, msg.clone()).await;
                                        continue;
                                    } else {
                                        send_media_group(client, target.clone(), vec![msg.clone()]).await?;
                                        continue;
                                    } 
                                } else if !msg.text().is_empty() {
                                    let mse = InputMessage::text(msg.text());
                                    client.send_message(target.clone(), mse).await?;
                                    continue;
                                }
                            };
                        } else {
                            println!("Канал не в списке");
                        }
                    }
                    _ => {}
                }
            }
            d => {}, 
        }
    }
}

async fn resolve_chnnels(client: &mut Client, channels: Vec<String>) -> Result<Vec<Channel>> {
    let mut channels_chat = Vec::new();

    for name in channels {
        match client.resolve_username(&name).await {
            Ok(Some(chat)) => match chat {
                Chat::Channel(ch) => {
                    println!("Поучаем resolve: {}", ch.title());
                    channels_chat.push(ch);
                },
                _ => {
                    // Ничего кроме каналов не добавляем
                }
            },
            Ok(None) => println!("Не удалось найти чат: {}", name),
            Err(e) => println!("Не удалось получить чат канала: {}\nОшибка: {:?}", name, e),
        }
        sleep(Duration::from_secs(2)).await;
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

    let config = crate::config::Config::load_config().await?;

    let init_config = config.clone();
    let api_id = init_config.main_config.app_id;
    let api_hash = init_config.main_config.api_hash;
    let target = init_config.bot_settings.target_channel;
    let session_file_name = init_config.main_config.session_file_name;
    let session_file_name = format!("{}.session", session_file_name);
    let channels = init_config.bot_settings.source_channels;
    let mistral_token = init_config.main_config.mistral_token;

    println!("{:#?}", config);

    let mut client = login::login(api_id, api_hash, &session_file_name).await;
    let chated = resolve_chnnels(&mut client, channels.clone()).await?;
    println!("Каналов найдено: {}", chated.len());
    // join_channels(&mut client, &chated).await;
    if let Err(e) = monitor_and_forward(&mut client, &target, chated, &mistral_token).await {
        eprintln!("Error from monitor_and_forward: {:?}", e)
    };

    Ok(())
}
