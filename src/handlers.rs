use std::{collections::HashMap, error::Error, sync::Arc, time::Duration};

use grammers_client::types::Message;
use tokio::{sync::Mutex, time::Instant};

use crate::mistral::{MistralClient, MistralResponse};

/// Структура для обработки Media
#[derive(Debug, Clone)]
pub struct MediaGroupHandler {
    /// Мутекс HashMap хранящий grouped_id и вектор сообщений
    pub groups: Arc<Mutex<HashMap<i64, Vec<Message>>>>,
    /// Метка. Когда последгий раз был добавлено Media. 
    /// Т.к альбом получается в client.next_update() последовательными обновлениями с минимальными задержками
    pub last_seen: Arc<Mutex<HashMap<i64, Instant>>>,
    /// Время ожидания Media
    pub timeout: Duration,
}

impl MediaGroupHandler {
    /// Инициализация структуры медиа группы
    pub async fn new(timeout: Duration) -> Self {
        Self {
            groups: Arc::new(Mutex::new(HashMap::new())),
            last_seen: Arc::new(Mutex::new(HashMap::new())),
            timeout,
        }
    }

    /// Добавляем/Создаём HashMap альбома GroupedID
    /// Фиксируем last_seen
    pub async fn add_message(&self, group_id: i64, message: Message) {
        let mut groups = self.groups.lock().await;
        let mut last_seen = self.last_seen.lock().await;

        groups.entry(group_id).or_default().push(message);
        last_seen.insert(group_id, Instant::now());
    }

    /// Получаем все группы, у которых вышел timeout
    pub async fn get_expired_groups(&self) -> Vec<(i64, Vec<Message>)> {
        let now = Instant::now();
        let mut expired = Vec::new();

        let mut groups = self.groups.lock().await;
        let mut last_seen = self.last_seen.lock().await;

        last_seen.retain(|&group_id, &mut seen| {
            if now.duration_since(seen) >= self.timeout {
                if let Some(messages) = groups.remove(&group_id) {
                    expired.push((group_id, messages));
                }
                false
            } else {
                true
            }
        });

        expired
    }
}

pub async fn generate(text: &str) -> Result<MistralResponse, Box<dyn Error>> {
    let api_url = "https://api.mistral.ai/v1/chat/completions".to_string();
    let model = "pixtral-large-latest".to_string();
    let client = MistralClient::new(&api_url);

    let system_prompt = r#"
        Ты — помощник по программированию, который помогает разработчикам обрабатывать текстовые сообщения и фильтровать их на основе заданных критериев. Тебе нужно анализировать входной текст и выдавать структурированный ответ, который можно использовать в коде.

1. Если текст содержит ключевые слова из заданного списка или имеет схожую тематику по словам, отметь его как "релевантный".
2. Если текст содержит рекламу или ссылки, отметь его как "реклама".
3. Если текст не соответствует ни одному из критериев, отметь его как "не релевантный".
4. Если текст похож на новость, то отметь его как релевантный.
5. Если текст слишком длинный, более 1000 символов, то сделай выжимку из текста, более сжатым и понятным языком вместо оригинального текст, также используй эмодзи.
Список: ИИ, Нейросети, утилита, утилиты, сервис, новости, новость, модель, OpenAI, Google, Mistral, Gemini, ChatGPT, GPT, DeepSeek, Grok, Elon Mask, технологии
Формат ответа:
{
    "status": "релевантный" | "реклама" | "не релевантный",
    "text": "оригинальный текст"
}

ВАЖНО: Ответ должен начинаться с "{" заканчиваться на "}"

Пример входного текста: "Скидка 50% на все товары! Посетите наш сайт: http://example.com"
    "#;
    let temperature = 0.7;

    match client.get_response(&model, temperature, text, system_prompt).await {
        Ok(res) => {
            // println!("Status: {:#?}", res.choices.iter().map(|ms| ms.message.content.clone()).collect::<Vec<String>>())
            return Ok(res);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            return Err(e);
        }
    }
}