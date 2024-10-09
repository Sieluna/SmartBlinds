use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::extract::{
    ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    State,
};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use lumisync_api::Message;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot};
use uuid::Uuid;

use super::MessageProtocol;

/// WebSocket Protocol Adapter
#[derive(Debug)]
pub struct WebSocketProtocol {
    /// WebSocket server address
    addr: SocketAddr,
    /// Client connection management
    clients: Arc<Mutex<HashMap<String, mpsc::Sender<WsMessage>>>>,
    /// Application message broadcast channel
    app_tx: Arc<broadcast::Sender<Message>>,
    /// Stop server sender
    stop_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl WebSocketProtocol {
    pub fn new(addr: SocketAddr) -> Self {
        let (app_tx, _) = broadcast::channel(100);
        Self {
            addr,
            clients: Arc::new(Mutex::new(HashMap::new())),
            app_tx: Arc::new(app_tx),
            stop_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Handle new WebSocket connection
    async fn handle_socket(
        socket: WebSocket,
        client_id: String,
        clients: Arc<Mutex<HashMap<String, mpsc::Sender<WsMessage>>>>,
        app_tx: Arc<broadcast::Sender<Message>>,
    ) {
        let (mut sender, mut receiver) = socket.split();

        // Create message channel for the client
        let (client_tx, mut client_rx) = mpsc::channel::<WsMessage>(100);

        // Add client to connection pool
        {
            let mut clients_map = clients.lock().unwrap();
            clients_map.insert(client_id.clone(), client_tx);
        }

        // Subscribe to application message broadcast channel
        let mut app_rx = app_tx.subscribe();

        // Task to send messages to the client
        let send_task = tokio::spawn(async move {
            while let Some(msg) = client_rx.recv().await {
                if sender.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Task to handle broadcast messages
        let clients_for_broadcast = clients.clone();
        let client_id_for_broadcast = client_id.clone();
        let broadcast_task = tokio::spawn(async move {
            while let Ok(app_msg) = app_rx.recv().await {
                // Serialize AppMessage to JSON
                if let Ok(json) = serde_json::to_string(&app_msg) {
                    // Get sender to avoid holding lock during await
                    let tx = {
                        let clients_guard = clients_for_broadcast.lock().unwrap();
                        clients_guard.get(&client_id_for_broadcast).cloned()
                    };

                    // If sender found, send message
                    if let Some(tx) = tx {
                        let _ = tx.send(WsMessage::Text(json)).await;
                    }
                }
            }
        });

        // Handle messages from client
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                WsMessage::Text(text) => {
                    // Parse JSON message from client as Message
                    if let Ok(app_msg) = serde_json::from_str::<Message>(&text) {
                        // Broadcast to other clients
                        let _ = app_tx.send(app_msg);
                    }
                }
                WsMessage::Binary(bin) => {
                    // Parse binary message from client as Message
                    if let Ok(app_msg) = bincode::deserialize::<Message>(&bin) {
                        // Broadcast to other clients
                        let _ = app_tx.send(app_msg);
                    }
                }
                WsMessage::Close(_) => break,
                _ => {}
            }
        }

        // Client disconnected, clean up resources
        {
            let mut clients_map = clients.lock().unwrap();
            clients_map.remove(&client_id);
        }

        // Stop tasks
        send_task.abort();
        broadcast_task.abort();
    }

    /// WebSocket upgrade handler
    async fn ws_handler(
        ws: WebSocketUpgrade,
        State((clients, app_tx)): State<(
            Arc<Mutex<HashMap<String, mpsc::Sender<WsMessage>>>>,
            Arc<broadcast::Sender<Message>>,
        )>,
    ) -> impl IntoResponse {
        let client_id = Uuid::new_v4().to_string();
        let clients_clone = clients.clone();
        let app_tx_clone = app_tx.clone();
        let client_id_clone = client_id.clone();

        ws.on_upgrade(move |socket| {
            Self::handle_socket(socket, client_id_clone, clients_clone, app_tx_clone)
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
            .with_state((self.clients.clone(), self.app_tx.clone()));

        let listener = TcpListener::bind(&self.addr).await?;
        tracing::info!("WebSocket server listening on {}", self.addr);

        // Create stop signal channel
        let (stop_tx, stop_rx) = oneshot::channel();
        {
            let mut stop_tx_guard = self.stop_tx.lock().unwrap();
            *stop_tx_guard = Some(stop_tx);
        }

        // Start server
        let server = axum::serve(listener, app);
        let graceful = server.with_graceful_shutdown(async {
            stop_rx.await.ok();
        });

        // Run server in new task
        tokio::spawn(async move {
            if let Err(e) = graceful.await {
                tracing::error!("WebSocket server error: {}", e);
            }
        });

        Ok(())
    }

    async fn send_app_message(&self, message: Message) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Send message through broadcast channel
        let _ = self.app_tx.send(message);
        Ok(())
    }

    async fn send_device_message(
        &self,
        _message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // WebSocket protocol doesn't directly handle device messages
        // If needed, could convert to AppMessage and send
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Send stop signal
        let mut stop_tx_guard = self.stop_tx.lock().unwrap();
        if let Some(tx) = stop_tx_guard.take() {
            let _ = tx.send(());
        }

        // Clean up client connections
        let mut clients = self.clients.lock().unwrap();
        clients.clear();

        Ok(())
    }
}
