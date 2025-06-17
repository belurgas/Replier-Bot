use std::{io::{self, BufRead, Write}, ops::ControlFlow, time::Duration};

use grammers_client::{Client, Config, InitParams, ReconnectionPolicy, SignInError};
use grammers_session::Session;

use crate::{log_error, log_info};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn prompt(message: &str) -> Result<String> {
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

struct MyPolicy;

impl ReconnectionPolicy for MyPolicy {
    ///this is the only function you need to implement,
    /// it gives you the attempted reconnections, and `self` in case you have any data in your struct.
    /// you should return a [`ControlFlow`] which can be either `Break` or `Continue`, break will **NOT** attempt a reconnection,
    /// `Continue` **WILL** try to reconnect after the given **Duration**.
    ///
    /// in this example we are simply sleeping exponentially based on the attempted count,
    /// however this is not a really good practice for production since we are just doing 2 raised to the power of attempts and that will result to massive
    /// numbers very soon, just an example!
    fn should_retry(&self, attempts: usize) -> ControlFlow<(), Duration> {
        if attempts >= 5 {
            return ControlFlow::Break(()); // Прекращаем попытки
        }
        log_info!("Reconect...");

        // Вычисляем время ожидания с ограничением
        let duration = Duration::from_millis((2u64.pow(attempts as u32)).min(30000 as u64));
        ControlFlow::Continue(duration)
    }
}

pub async fn login(api_id: i32, api_hash: String, session_file: &str) -> Client {
    let config = Config {
        session: Session::load_file_or_create(session_file).unwrap(),
        api_id,
        api_hash,
        params: InitParams {
            reconnection_policy: &MyPolicy,
            flood_sleep_threshold: 0,
            update_queue_limit: Some(128),
            ..Default::default()
        },
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
        log_info!("Access successful!");
        match client.session().save_to_file(session_file) {
            Ok(_) => {}
            Err(e) => {
                log_error!("NOTE: failed to save the session, will sign out when done: {e}");
            }
        }
    }

    client
}