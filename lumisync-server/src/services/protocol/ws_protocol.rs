use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use lumisync_api::{
    Message,
    transport::{Protocol, deserialize, serialize},
};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot};
use uuid::Uuid;

use super::MessageProtocol;

#[derive(Debug)]
pub struct WebSocketProtocol {
    /// WebSocket server address
    addr: SocketAddr,
    /// Client connection management
    clients: Arc<RwLock<HashMap<String, mpsc::Sender<WsMessage>>>>,
    /// Application message broadcast channel
    app_tx: Arc<broadcast::Sender<Message>>,
    /// Stop server sender
    stop_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    /// Serialization protocol
    protocol: Protocol,
}

impl WebSocketProtocol {
    pub fn new(addr: SocketAddr) -> Self {
        let (app_tx, _) = broadcast::channel(1000);
        Self {
            addr,
            clients: Arc::new(RwLock::new(HashMap::new())),
            app_tx: Arc::new(app_tx),
            stop_tx: Arc::new(Mutex::new(None)),
            protocol: Protocol::default(),
        }
    }

    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    async fn handle_socket(
        socket: WebSocket,
        client_id: String,
        clients: Arc<RwLock<HashMap<String, mpsc::Sender<WsMessage>>>>,
        app_tx: Arc<broadcast::Sender<Message>>,
        protocol: Protocol,
    ) {
        let (mut sender, mut receiver) = socket.split();

        tracing::info!("WebSocket client {} connected", client_id);

        let (client_tx, mut client_rx) = mpsc::channel::<WsMessage>(1000);

        {
            let mut clients_map = clients.write().unwrap();
            clients_map.insert(client_id.clone(), client_tx);
            tracing::debug!("Active WebSocket connections: {}", clients_map.len());
        }

        let mut app_rx = app_tx.subscribe();

        let client_id_for_send = client_id.clone();
        let send_task = tokio::spawn(async move {
            while let Some(msg) = client_rx.recv().await {
                if let Err(e) = sender.send(msg).await {
                    tracing::warn!(
                        "Failed to send message to client {}: {}",
                        client_id_for_send,
                        e
                    );
                    break;
                }
            }
        });

        let clients_for_broadcast = clients.clone();
        let client_id_for_broadcast = client_id.clone();
        let broadcast_task = tokio::spawn(async move {
            while let Ok(app_msg) = app_rx.recv().await {
                // Use new serialization API consistently
                let message_data = match protocol {
                    Protocol::Json => match serialize(Protocol::Json, &app_msg) {
                        Ok(data) => match String::from_utf8(data) {
                            Ok(json_str) => WsMessage::Text(json_str),
                            Err(_) => {
                                tracing::error!(
                                    "Failed to convert JSON bytes to string for client {}",
                                    client_id_for_broadcast
                                );
                                continue;
                            }
                        },
                        Err(e) => {
                            tracing::error!(
                                "Failed to serialize JSON message for client {}: {:?}",
                                client_id_for_broadcast,
                                e
                            );
                            continue;
                        }
                    },
                    Protocol::Postcard => match serialize(Protocol::Postcard, &app_msg) {
                        Ok(data) => WsMessage::Binary(data),
                        Err(e) => {
                            tracing::error!(
                                "Failed to serialize binary message for client {}: {:?}",
                                client_id_for_broadcast,
                                e
                            );
                            continue;
                        }
                    },
                };

                let tx = {
                    let clients_guard = clients_for_broadcast.read().unwrap();
                    clients_guard.get(&client_id_for_broadcast).cloned()
                };

                if let Some(tx) = tx {
                    if let Err(e) = tx.send(message_data).await {
                        tracing::warn!(
                            "Failed to send broadcast message to client {}: {:?}",
                            client_id_for_broadcast,
                            e
                        );
                    }
                }
            }
        });

        while let Some(result) = receiver.next().await {
            match result {
                Ok(msg) => match msg {
                    WsMessage::Text(text) => {
                        // Try to deserialize as JSON first
                        match deserialize::<Message>(Protocol::Json, text.as_bytes()) {
                            Ok(app_msg) => {
                                if let Err(e) = app_tx.send(app_msg) {
                                    tracing::warn!(
                                        "Failed to broadcast text message from client {}: {:?}",
                                        client_id,
                                        e
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to deserialize text message from client {}: {:?}",
                                    client_id,
                                    e
                                );
                            }
                        }
                    }
                    WsMessage::Binary(bin) => {
                        // Use configured protocol for binary messages
                        match deserialize::<Message>(protocol, &bin) {
                            Ok(app_msg) => {
                                if let Err(e) = app_tx.send(app_msg) {
                                    tracing::warn!(
                                        "Failed to broadcast binary message from client {}: {:?}",
                                        client_id,
                                        e
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to deserialize binary message from client {}: {:?}",
                                    client_id,
                                    e
                                );
                            }
                        }
                    }
                    WsMessage::Close(reason) => {
                        tracing::info!(
                            "Client {} closed connection: {:?}",
                            client_id,
                            reason.map(|r| format!("{}: {}", r.code, r.reason))
                        );
                        break;
                    }
                    WsMessage::Ping(data) => {
                        let tx = {
                            let clients_map = clients.read().unwrap();
                            clients_map.get(&client_id).cloned()
                        };

                        if let Some(tx) = tx {
                            if let Err(e) = tx.send(WsMessage::Pong(data)).await {
                                tracing::warn!(
                                    "Failed to send pong to client {}: {}",
                                    client_id,
                                    e
                                );
                                break;
                            }
                        }
                    }
                    _ => {}
                },
                Err(e) => {
                    tracing::warn!("WebSocket error for client {}: {}", client_id, e);
                    break;
                }
            }
        }

        {
            let mut clients_map = clients.write().unwrap();
            clients_map.remove(&client_id);
            tracing::debug!("Remaining WebSocket connections: {}", clients_map.len());
        }

        send_task.abort();
        broadcast_task.abort();

        tracing::info!("Client {} disconnected", client_id);
    }

    async fn ws_handler(
        ws: WebSocketUpgrade,
        State((clients, app_tx, protocol)): State<(
            Arc<RwLock<HashMap<String, mpsc::Sender<WsMessage>>>>,
            Arc<broadcast::Sender<Message>>,
            Protocol,
        )>,
    ) -> impl IntoResponse {
        let client_id = Uuid::new_v4().to_string();
        let clients_clone = clients.clone();
        let app_tx_clone = app_tx.clone();
        let client_id_clone = client_id.clone();

        ws.on_upgrade(move |socket| {
            Self::handle_socket(
                socket,
                client_id_clone,
                clients_clone,
                app_tx_clone,
                protocol,
            )
        })
    }
}

#[async_trait]
impl MessageProtocol for WebSocketProtocol {
    fn name(&self) -> &'static str {
        "websocket"
    }

    async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let app = Router::new()
            .route("/ws", get(Self::ws_handler))
            .with_state((self.clients.clone(), self.app_tx.clone(), self.protocol));

        let listener = TcpListener::bind(&self.addr).await.map_err(|e| {
            tracing::error!("Failed to bind WebSocket server to {}: {}", self.addr, e);
            Box::new(e) as Box<dyn Error + Send + Sync>
        })?;
        tracing::info!(
            "WebSocket server listening on {} with {:?} protocol",
            self.addr,
            self.protocol
        );

        let (stop_tx, stop_rx) = oneshot::channel();
        {
            let mut stop_tx_guard = self.stop_tx.lock().unwrap();
            *stop_tx_guard = Some(stop_tx);
        }

        let server = axum::serve(listener, app);
        let graceful = server.with_graceful_shutdown(async {
            stop_rx.await.ok();
            tracing::info!("WebSocket server shutdown signal received");
        });

        tokio::spawn(async move {
            if let Err(e) = graceful.await {
                tracing::error!("WebSocket server error: {}", e);
            }
        });

        Ok(())
    }

    async fn send_app_message(&self, message: Message) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Err(e) = self.app_tx.send(message) {
            tracing::warn!("Failed to broadcast application message: {:?}", e);
        }
        Ok(())
    }

    async fn send_device_message(
        &self,
        _message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        tracing::debug!("WebSocket protocol doesn't handle device messages directly");
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        tracing::info!("Stopping WebSocket protocol server");

        let mut stop_tx_guard = self.stop_tx.lock().unwrap();
        if let Some(tx) = stop_tx_guard.take() {
            if let Err(e) = tx.send(()) {
                tracing::warn!("Failed to send WebSocket server stop signal: {:?}", e);
            }
        }

        let mut clients = self.clients.write().unwrap();
        let count = clients.len();
        clients.clear();
        tracing::info!("Closed {} WebSocket client connections", count);

        Ok(())
    }
}
