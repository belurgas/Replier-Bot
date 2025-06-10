use std::{collections::HashMap, error::Error, sync::Arc, time::Duration};

use grammers_client::{InputMedia};
use tokio::{sync::Mutex, time::Instant};

use crate::mistral::{MistralClient, MistralResponse};

/// Структура для обработки Media
#[derive(Clone)]
pub struct MediaGroupHandler {
    /// Мутекс HashMap хранящий grouped_id и вектор сообщений
    pub groups: Arc<Mutex<HashMap<i64, Vec<InputMedia>>>>,
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
    pub async fn add_media(&self, group_id: i64, media: InputMedia) {
        let mut groups = self.groups.lock().await;
        let mut last_seen = self.last_seen.lock().await;

        groups.entry(group_id).or_default().push(media);
        last_seen.insert(group_id, Instant::now());
    }

    /// Получаем все группы, у которых вышел timeout
    pub async fn get_expired_groups(&self) -> Vec<(i64, Vec<InputMedia>)> {
        let now = Instant::now();
        let mut expired = Vec::new();

        let mut groups = self.groups.lock().await;
        let mut last_seen = self.last_seen.lock().await;

        last_seen.retain(|&group_id, &mut seen| {
            if now.duration_since(seen) >= self.timeout {
                if let Some(media) = groups.remove(&group_id) {
                    expired.push((group_id, media));
                }
                false
            } else {
                true
            }
        });

        expired
    }
}

pub async fn generate(text: &str, mistral_token: &str) -> Result<MistralResponse, Box<dyn Error>> {
    let api_url = "https://api.mistral.ai/v1/chat/completions".to_string();
    let model = "pixtral-large-latest".to_string();
    let client = MistralClient::new(&api_url);

    let system_prompt = r#"
Ты — высокоэффективный помощник по программированию, предназначенный для анализа и фильтрации текстовых сообщений. Твоя задача — обрабатывать входные тексты и выдавать структурированные ответы, которые могут быть использованы в коде. 

Пожалуйста, следуй этим критериям:

1. Если текст содержит ключевые слова из заданного списка или имеет схожую тематику, отметь его как "релевантный".
2. Если текст содержит рекламу, ссылки или призывы к действию, отметь его как "реклама".
3. Если текст не соответствует ни одному из критериев, отметь его как "не релевантный".
4. Если текст похож на новость или содержит актуальную информацию, отметь его как "релевантный".
5. Если текст превышает 1000 символов, сделай краткий пересказ, используя более сжатый и понятный язык, а также добавь эмодзи для улучшения восприятия.
6. Следуй стоп словам, если видишь их или похожие по тематике, то отмечай его как "не релевантны"
Список ключевых слов: ИИ, Нейросети, нейоронные сети, утилита, утилиты, сервис, модель, OpenAI, Google, Mistral, Gemini, ChatGPT, GPT, DeepSeek, Grok, Elon Musk, технологии, мемы, 
Стоп слова: политика, война, пропоганда, погода, новости не касаюшиеся ИИ.
Формат ответа строго такой:
{
    "status": "релевантный" | "реклама" | "не релевантный",
    "text": "оригинальный текст или сжатый пересказ"
}

ВАЖНО: Ответ должен начинаться с "{" и заканчиваться на "}". НИ В КОЕМ СЛУЧАЕ НЕ ОТВЕЧАЙ С ФОРМАТИРОВАНИЕМ. Это системный ответ, который пользователь не видит.

Пример входного текста: "Скидка 50% на все товары! Посетите наш сайт: http://example.com"
"#;
    let temperature = 0.7;

    match client.get_response(&model, temperature, text, system_prompt, mistral_token).await {
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