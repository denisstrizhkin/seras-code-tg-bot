use std::collections::HashMap;

use ollama_rs::generation::chat::ChatMessage;
use teloxide::types::ChatId;

type Messages = std::sync::Arc<std::sync::Mutex<Vec<ChatMessage>>>;
type MessagesHashMap = HashMap<ChatId, Messages>;

#[derive(Default)]
pub struct History {
    messages: tokio::sync::RwLock<MessagesHashMap>,
}

impl History {
    pub async fn get<'a>(&'a self, chat_id: ChatId) -> ChatHistory<'a> {
        self.messages.write().await.entry(chat_id).or_default();
        let guard = self.messages.read().await;
        let messages = guard.get(&chat_id).unwrap().clone();
        ChatHistory { guard, messages }
    }

    pub async fn clear(&self, chat_id: ChatId) {
        if let Some(messages) = self.messages.read().await.get(&chat_id) {
            messages.lock().unwrap().clear();
        }
    }
}

pub struct ChatHistory<'a> {
    guard: tokio::sync::RwLockReadGuard<'a, MessagesHashMap>,
    pub messages: Messages,
}
