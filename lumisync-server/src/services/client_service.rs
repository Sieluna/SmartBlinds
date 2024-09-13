use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use dashmap::{DashMap, DashSet};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use tokio::sync::mpsc;

use super::event_bus::EventBus;

static CLIENT_ID_GENERATOR: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Command {
        action: String,
        params: Option<serde_json::Value>,
        timestamp: OffsetDateTime,
    },
    Event {
        event_type: String,
        payload: serde_json::Value,
        timestamp: OffsetDateTime,
    },
    Ack {
        status: u16,
        result: serde_json::Value,
        timestamp: OffsetDateTime,
    },
    Error {
        code: u16,
        message: String,
        timestamp: OffsetDateTime,
    },
}

#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub client_id: usize,
    pub user_id: Option<i32>,
    pub subscriptions: DashSet<String>,
    pub connected_at: OffsetDateTime,
    pub last_activity: OffsetDateTime,
}

#[derive(Clone)]
pub struct ClientService {
    event_bus: Arc<EventBus>,
    clients: Arc<DashMap<usize, ClientConnection>>,
}

impl ClientService {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            clients: Arc::new(DashMap::new()),
        }
    }

    /// Main entry for each WebSocket connection.
    pub async fn handle_connection(&self, user_id: Option<i32>, ws: WebSocket) {
        let client_id = CLIENT_ID_GENERATOR.fetch_add(1, Ordering::SeqCst);
        let now = OffsetDateTime::now_utc();

        self.clients.insert(
            client_id,
            ClientConnection {
                client_id,
                user_id,
                subscriptions: DashSet::new(),
                connected_at: now,
                last_activity: now,
            },
        );

        let (mut sender, mut receiver) = ws.split();
        let (tx, mut rx) = mpsc::channel::<Message>(100);

        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    if let Err(e) = sender.send(WsMessage::Text(json)).await {
                        tracing::error!("Failed to send message: {e}");
                        break;
                    }
                }
            }
        });

        let receive_task = {
            let manager = self.clone();
            let tx = tx.clone();

            tokio::spawn(async move {
                while let Some(Ok(msg)) = receiver.next().await {
                    if let WsMessage::Text(text) = msg {
                        if let Some(mut client) = manager.clients.get_mut(&client_id) {
                            client.last_activity = OffsetDateTime::now_utc();
                        }

                        if let Err(e) = manager.handle_message(&text, client_id, &tx).await {
                            let error_msg = Message::Error {
                                code: 400,
                                message: e.to_string(),
                                timestamp: OffsetDateTime::now_utc(),
                            };
                            let _ = tx.send(error_msg).await;
                        }
                    }
                }

                manager.clients.remove(&client_id);
            })
        };

        tokio::select! {
            _ = send_task => {},
            _ = receive_task => {},
        }
    }

    /// Parse, route and execute incoming client messages.
    async fn handle_message(
        &self,
        text: &str,
        client_id: usize,
        tx: &mpsc::Sender<Message>,
    ) -> Result<(), String> {
        let msg: Message =
            serde_json::from_str(text).map_err(|e| format!("invalid JSON: {}", e))?;

        if let Message::Command { action, params, .. } = msg {
            match action.as_str() {
                "subscribe" => {
                    let event_type = params
                        .as_ref()
                        .and_then(|v| v.get("event_type"))
                        .and_then(|v| v.as_str())
                        .ok_or("Missing event_type")?
                        .to_owned();

                    if let Some(client) = self.clients.get_mut(&client_id) {
                        client.subscriptions.insert(event_type.to_string());
                    }

                    let mut rx = self.event_bus.subscribe(&event_type).await;
                    let tx_clone = tx.clone();
                    let event_type_clone = event_type.clone();
                    tokio::spawn(async move {
                        while let Ok(event) = rx.recv().await {
                            let msg = Message::Event {
                                event_type: event_type_clone.clone(),
                                payload: serde_json::to_value(event).unwrap(),
                                timestamp: OffsetDateTime::now_utc(),
                            };
                            let _ = tx_clone.send(msg).await;
                        }
                    });

                    tx.send(Message::Ack {
                        status: 200,
                        result: json!({ "event_type": event_type.to_string() }),
                        timestamp: OffsetDateTime::now_utc(),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                }
                "unsubscribe" => {
                    let event_type = params
                        .as_ref()
                        .and_then(|v| v.get("event_type"))
                        .and_then(|v| v.as_str())
                        .ok_or("Missing event_type")?;

                    if let Some(client) = self.clients.get_mut(&client_id) {
                        client.subscriptions.remove(event_type);
                    }

                    tx.send(Message::Ack {
                        status: 200,
                        result: json!({ "event_type": event_type }),
                        timestamp: OffsetDateTime::now_utc(),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                }
                "ping" => {
                    tx.send(Message::Ack {
                        status: 200,
                        result: json!({ "message": "pong" }),
                        timestamp: OffsetDateTime::now_utc(),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                }
                other => {
                    tx.send(Message::Error {
                        code: 400,
                        message: format!("Unknown action: {}", other),
                        timestamp: OffsetDateTime::now_utc(),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                    return Err(format!("Unknown action: {}", other));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::services::event_bus::EventPayload;

    use super::*;

    #[tokio::test]
    async fn test_event_bus_integration() {
        let bus = Arc::new(EventBus::new());
        let svc = ClientService::new(bus.clone());
        let client_id = 1;
        svc.clients.insert(
            client_id,
            ClientConnection {
                client_id,
                user_id: None,
                subscriptions: DashSet::new(),
                connected_at: OffsetDateTime::now_utc(),
                last_activity: OffsetDateTime::now_utc(),
            },
        );
        let (tx, mut rx) = mpsc::channel(10);
        let evt = "test.event";

        // subscribe
        svc.handle_message(
            &serde_json::to_string(&Message::Command {
                action: "subscribe".into(),
                params: Some(json!({ "event_type": evt })),
                timestamp: OffsetDateTime::now_utc(),
            })
            .unwrap(),
            client_id,
            &tx,
        )
        .await
        .unwrap();
        if let Message::Ack { result, .. } = rx.recv().await.unwrap() {
            assert_eq!(result["event_type"], evt);
        } else {
            panic!("Expected Ack");
        }

        // publish + receive
        assert!(
            bus.publish(
                evt,
                EventPayload::Generic {
                    event_type: evt.into(),
                    data: "data".into(),
                    timestamp: OffsetDateTime::now_utc(),
                }
            )
            .await
            .unwrap()
                > 0
        );
        if let Message::Event {
            event_type,
            payload,
            ..
        } = rx.recv().await.unwrap()
        {
            assert_eq!(event_type, evt);
            assert_eq!(payload["Generic"]["data"], "data");
        } else {
            panic!("Expected Event");
        }

        // unsubscribe
        svc.handle_message(
            &serde_json::to_string(&Message::Command {
                action: "unsubscribe".into(),
                params: Some(json!({ "event_type": evt })),
                timestamp: OffsetDateTime::now_utc(),
            })
            .unwrap(),
            client_id,
            &tx,
        )
        .await
        .unwrap();
        if let Message::Ack { result, .. } = rx.recv().await.unwrap() {
            assert_eq!(result["event_type"], evt);
        }
        assert!(!svc
            .clients
            .get(&client_id)
            .unwrap()
            .subscriptions
            .contains(evt));
    }
}
