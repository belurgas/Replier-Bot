use dotenv::dotenv;
use grammers_client::types::{Chat, Downloadable, Media, Message};
use grammers_client::{Client, InputMedia, InputMessage, Update};
use tokio::time::interval;
use std::time::Duration;

use crate::handlers::MediaGroupHandler;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod handlers;
mod config;
mod login;

async fn join_channels(client: &mut Client, channels: &Vec<String>) {
    for username in channels {
        match client.resolve_username(username).await {
            Ok(chat) => if let Some(chat) = chat {
                if let Err(e) = client.join_chat(chat).await {
                    println!("Не удалось подписаться на {}: {:?}", username, e);
                } else {
                    println!("Подписался на {}", username);
                }
            }
            _ => println!("Не удалось найти канал: {}", username),
        }
    }
}

async fn send_media_group(client: &mut Client, target_channel: Chat, messages: Vec<Message>) -> Result<()> {
    
    let mut album: Vec<InputMedia> = Vec::with_capacity(messages.len());

    // Отправляем совокупные медиа
    for (idx, i) in messages.iter().enumerate() {
        match i.chat() {
            Chat::Channel(ch) => {
                if let Some(media) = i.media() {
                    if ch.raw.noforwards {
                        if !i.text().is_empty() {
                            let path = format!("./image_{}.jpg", idx);
                            let down = Downloadable::Media(media);
                            client.download_media(&down, &path).await?;
                            let uploaded = client.upload_file(path).await?;
                            client.send_message(target_channel, InputMessage::text(i.text()).photo(uploaded)).await?;
                            return Ok(());
                        }
                        println!("123");
                        return Ok(());
                    } else {    
                        let caption = i.text().to_string();
                        let input_meda = InputMedia::caption(caption)
                            .copy_media(&media);

                        album.push(input_meda);
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

async fn monitor_and_forward(client: &mut Client, target_channel: &str) -> Result<()> {
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
                        // Значит получили обновление из таргет канал
                        continue;
                    }
                }

                match msg.chat() {
                    Chat::Channel(ch) => {
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
                                            }
                                        }
                                        Media::Photo(phot) => {
                                            path = "./image.jpg".to_string();
                                        },
                                        d => {
                                            println!("Media: {:#?}", d);
                                        }
                                    }
                                    let down = Downloadable::Media(media.clone());
                                    client.download_media(&down, &path).await?;

                                    let uploaded = client.upload_file(&path).await?;

                                    match media.clone() {
                                        Media::Document(_) => {
                                            client.send_message(target.clone(), InputMessage::text(msg.text()).document(uploaded)).await?;
                                        },
                                        Media::Photo(_) => {
                                            client.send_message(target.clone(), InputMessage::text(msg.text()).photo(uploaded)).await?;
                                        }
                                        _ => {}
                                    }
                                } else {
                                    client.send_message(target.clone(), InputMessage::text(msg.text())).await?;
                                }
                                continue;
                            }
                            continue;
                        }
                    }
                    _ => {}
                }

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
            }
            d => {}, 
        }
    }
}


#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let config = crate::config::Config::load_config().await?;

    let init_config = config.clone();
    let api_id = init_config.main_config.app_id;
    let api_hash = init_config.main_config.api_hash;
    let channels = init_config.bot_settings.source_channels;
    let target = init_config.bot_settings.target_channel;
    let session_file_name = init_config.main_config.session_file_name;
    let session_file_name = format!("{}.session", session_file_name);

    println!("{:#?}", config);

    let mut client = login::login(api_id, api_hash, &session_file_name).await;
    join_channels(&mut client, &channels).await;
    if let Err(e) = monitor_and_forward(&mut client, &target).await {
        eprintln!("Error from monitor_and_forward: {:?}", e)
    };

    Ok(())
}
