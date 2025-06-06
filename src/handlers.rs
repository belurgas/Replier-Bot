use std::{collections::HashMap, sync::Arc, time::Duration};

use grammers_client::types::Message;
use tokio::{sync::Mutex, time::Instant};

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