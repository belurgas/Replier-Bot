use std::io::{self, BufRead, Write};

use grammers_client::{Client, Config, SignInError};
use grammers_session::Session;

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

pub async fn login(api_id: i32, api_hash: String, session_file: &str) -> Client {
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
        match client.session().save_to_file(session_file) {
            Ok(_) => {}
            Err(e) => {
                println!("NOTE: failed to save the session, will sign out when done: {e}");
            }
        }
    }

    client
}