use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use lumisync_api::Message;
use lumisync_api::transport::{Protocol, deserialize, serialize};
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::services::MessageRouter;

#[derive(Clone)]
pub struct WebSocketState {
    pub message_router: Arc<MessageRouter>,
    pub clients: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<WsMessage>>>>,
}

pub fn websocket_router(state: WebSocketState) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<WebSocketState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

async fn handle_websocket(socket: WebSocket, state: WebSocketState) {
    let client_id = Uuid::new_v4().to_string();
    let (mut sender, mut receiver) = socket.split();
    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<WsMessage>();

    {
        let mut clients = state.clients.write().await;
        clients.insert(client_id.clone(), client_tx);
    }

    let (subscription_id, mut app_messages) = state.message_router.subscribe_app_messages().await;

    tracing::info!("WebSocket client {} connected", client_id);

    let client_id_send = client_id.clone();
    let send_task = tokio::spawn(async move {
        while let Some(msg) = client_rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
        tracing::info!("WebSocket client {} send task ended", client_id_send);
    });

    let clients_broadcast = state.clients.clone();
    let client_id_broadcast = client_id.clone();
    let broadcast_task = tokio::spawn(async move {
        while let Some(app_message) = app_messages.recv().await {
            let ws_message = match serialize(Protocol::Json, &app_message) {
                Ok(data) => match String::from_utf8(data) {
                    Ok(json_str) => WsMessage::Text(json_str),
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            let tx = {
                let clients = clients_broadcast.read().await;
                clients.get(&client_id_broadcast).cloned()
            };

            if let Some(tx) = tx {
                if tx.send(ws_message).is_err() {
                    break;
                }
            }
        }
    });

    while let Some(result) = receiver.next().await {
        match result {
            Ok(WsMessage::Text(text)) => {
                match deserialize::<Message>(Protocol::Json, text.as_bytes()) {
                    Ok(message) => {
                        state.message_router.process_incoming_message(message).await;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to deserialize message: {:?}", e);
                    }
                }
            }
            Ok(WsMessage::Close(_)) => {
                tracing::info!("WebSocket client {} closed", client_id);
                break;
            }
            Err(e) => {
                tracing::warn!("WebSocket error for client {}: {}", client_id, e);
                break;
            }
            _ => {}
        }
    }

    send_task.abort();
    broadcast_task.abort();
    state.message_router.unsubscribe(&subscription_id).await;

    {
        let mut clients = state.clients.write().await;
        clients.remove(&client_id);
    }

    tracing::info!("WebSocket client {} disconnected", client_id);
}
