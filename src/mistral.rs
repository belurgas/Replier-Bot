use std::error::Error;

use reqwest::Client;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct MistralRequest {
    model: String,
    temperature: f32,
    messages: Vec<Message>,
}

#[derive(Deserialize, Default, Debug)]
pub struct MistralResponse {
    pub id: String,
    pub choices: Vec<Choice>,
}

#[derive(Deserialize, Default, Debug)]
pub struct Choice {
    pub index: i32,
    pub message: Message,
}

pub struct MistralClient {
    client: Client,
    api_url: String,
}

impl MistralClient {
    pub fn new(api_url: &str) -> Self {
        let client = Client::new();
        MistralClient {
            client,
            api_url: api_url.to_string(),
        }
    }

    pub async fn get_response(&self, model: &str, temperature: f32, input_text: &str, system_prompt: &str, mistral_token: &str) -> Result<MistralResponse, Box<dyn Error>> {
        println!("Отправляем ИИ запрос: {:#?}", input_text);
        
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: input_text.to_string(),
            },
        ];

        let request_body = MistralRequest {
            model: model.to_string(),
            temperature,
            messages,
        };

        let token = "29DAkIiKLknaPju4xFeghAbKpr5so1CC";
        let response = self.client
            .post(&self.api_url)
            .bearer_auth(token)
            .json(&request_body)
            .send()
            .await?;

        if response.status().is_success() {
            let data = response.json::<MistralResponse>().await?;
            println!("DATA: {:#?}", data);
            Ok(data)
        } else {
            Err(format!("Err: {}", response.status()).into())
        }
    }
}