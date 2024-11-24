mod tcp;
mod websocket;

pub use tcp::*;
pub use websocket::*;

use std::collections::HashMap;
use std::sync::Arc;

use lumisync_api::message::Message;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

pub struct MessageRouter {
    app_subscribers: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    device_subscribers: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    message_processor: mpsc::UnboundedSender<Message>,
}

impl MessageRouter {
    pub fn new(message_processor: mpsc::UnboundedSender<Message>) -> Self {
        Self {
            app_subscribers: Arc::new(RwLock::new(HashMap::new())),
            device_subscribers: Arc::new(RwLock::new(HashMap::new())),
            message_processor,
        }
    }

    pub async fn subscribe_app_messages(&self) -> (String, mpsc::UnboundedReceiver<Message>) {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::unbounded_channel();

        self.app_subscribers.write().await.insert(id.clone(), tx);
        (id, rx)
    }

    pub async fn subscribe_device_messages(&self) -> (String, mpsc::UnboundedReceiver<Message>) {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::unbounded_channel();

        self.device_subscribers.write().await.insert(id.clone(), tx);
        (id, rx)
    }

    pub async fn unsubscribe(&self, subscription_id: &str) {
        self.app_subscribers.write().await.remove(subscription_id);
        self.device_subscribers
            .write()
            .await
            .remove(subscription_id);
    }

    pub async fn publish_app_message(&self, message: Message) {
        let subscribers = self.app_subscribers.read().await;
        for (id, sender) in subscribers.iter() {
            if sender.send(message.clone()).is_err() {
                tracing::warn!("Failed to send message to app subscriber: {}", id);
            }
        }
    }

    pub async fn publish_device_message(&self, message: Message) {
        let subscribers = self.device_subscribers.read().await;
        for (id, sender) in subscribers.iter() {
            if sender.send(message.clone()).is_err() {
                tracing::warn!("Failed to send message to device subscriber: {}", id);
            }
        }
    }

    pub async fn process_incoming_message(&self, message: Message) {
        if self.message_processor.send(message).is_err() {
            tracing::error!("Failed to send message to processor");
        }
    }
}
