use dotenv::dotenv;
use grammers_client::types::{Chat, Downloadable, Message};
use grammers_client::{Client, Config, InputMedia, InputMessage, SignInError, Update};
use grammers_session::Session;
use tokio::time::interval;
use std::env;
use std::io::{self, BufRead as _, Write as _};
use std::time::Duration;

use crate::handlers::MediaGroupHandler;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SESSION_FILE: &str = "session.session";

mod handlers;
mod config;

fn prompt(message: &str) -> Result<String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(message.as_bytes())?;
    stdout.flush()?;

    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    let mut line = String::new();
    stdin.read_line(&mut line)?;
    Ok(line)
}

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
                        // Значит получили обновление из таргет канала
                        println!("1");
                        continue;
                    }
                }

                match msg.chat() {
                    Chat::Channel(ch) => {
                        if ch.raw.noforwards {
                            println!("Не возможно переслать");
                            if let Some(group_id) = msg.grouped_id() {
                                println!("Группа, собираем!");
                                group_handler.add_message(group_id, msg.clone()).await;
                                
                                
                            } else {
                                println!("Не группа, просто качаем и отправляем");
                                if let Some(media) = msg.media() {
                                    let down = Downloadable::Media(media.clone());
                                    client.download_media(&down, "./image.jpg").await?;

                                    let uploaded = client.upload_file("./image.jpg").await?;
                                    client.send_message(target.clone(), InputMessage::text(msg.text()).photo(uploaded)).await?;
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

async fn login(api_id: i32, api_hash: String) -> Client {
    let config = Config {
        session: Session::load_file_or_create("session.session").unwrap(),
        api_id,
        api_hash,
        params: Default::default(),
    };

    let client = Client::connect(config).await.unwrap();
    if !client.is_authorized().await.unwrap() {
        let phone = prompt("Введите номер телефона: ").unwrap();
        let token = client.request_login_code(&phone).await.unwrap();
        let code = prompt("Введите код из Tg: ").unwrap();
        let signed_in = client.sign_in(&token, &code).await;
        match signed_in {
            Err(SignInError::PasswordRequired(password_token)) => {
                let hint = password_token.hint().unwrap_or("None");
                let prompt_message = format!("Введённый пароль (hint {}): ", &hint);
                let password = prompt(prompt_message.as_str()).unwrap();

                client
                    .check_password(password_token, password.trim())
                    .await.unwrap();
            }
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        }
        println!("Мы внутри");
        match client.session().save_to_file(SESSION_FILE) {
            Ok(_) => {}
            Err(e) => {
                println!("NOTE: failed to save the session, will sign out when done: {e}");
            }
        }
    }

    client
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

    println!("{:#?}", config);

    let mut client = login(api_id, api_hash).await;
    join_channels(&mut client, &channels).await;
    if let Err(e) = monitor_and_forward(&mut client, &target).await {
        eprintln!("Error from monitor_and_forward: {:?}", e)
    };

    Ok(())
}
