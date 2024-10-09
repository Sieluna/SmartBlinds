use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lumisync_api::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, oneshot};

use super::MessageProtocol;

/// TCP Protocol Adapter
#[derive(Debug)]
pub struct TcpProtocol {
    /// TCP server address
    addr: SocketAddr,
    /// Device connection management
    devices: Arc<Mutex<HashMap<i32, mpsc::Sender<Vec<u8>>>>>,
    /// Device message broadcast channel
    device_tx: Arc<broadcast::Sender<Message>>,
    /// Stop server sender
    stop_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl TcpProtocol {
    pub fn new(addr: SocketAddr) -> Self {
        let (device_tx, _) = broadcast::channel(100);
        Self {
            addr,
            devices: Arc::new(Mutex::new(HashMap::new())),
            device_tx: Arc::new(device_tx),
            stop_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Handle new TCP connection
    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        devices: Arc<Mutex<HashMap<i32, mpsc::Sender<Vec<u8>>>>>,
        device_tx: Arc<broadcast::Sender<Message>>,
    ) {
        // In real scenarios, there should be an authentication mechanism
        let (mut reader, mut writer) = stream.into_split();

        // Read device ID (simplified example)
        let mut id_buffer = [0u8; 4];
        if let Err(e) = reader.read_exact(&mut id_buffer).await {
            tracing::error!("Failed to read device ID: {}", e);
            return;
        }

        let device_id = i32::from_be_bytes(id_buffer);
        tracing::info!("Device {} connected from {}", device_id, addr);

        // Create send channel
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(100);

        // Register device
        {
            let mut devices_map = devices.lock().unwrap();
            devices_map.insert(device_id, tx);
        }

        // Subscribe to device message broadcast
        let mut device_rx = device_tx.subscribe();

        // Task to send messages to device
        let send_task = tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                // Add length prefix
                let len = data.len() as u32;
                if writer.write_all(&len.to_be_bytes()).await.is_err() {
                    break;
                }

                if writer.write_all(&data).await.is_err() {
                    break;
                }
            }
        });

        // Task to handle broadcast messages
        let devices_for_broadcast = devices.clone();
        let device_id_for_broadcast = device_id;
        let broadcast_task = tokio::spawn(async move {
            while let Ok(device_frame) = device_rx.recv().await {
                // Forward based on target device ID (simplified here, in reality should extract target device ID from message)
                if let Ok(bin) = bincode::serialize(&device_frame) {
                    // Get sender to avoid holding lock during await
                    let tx = {
                        let devices_guard = devices_for_broadcast.lock().unwrap();
                        devices_guard.get(&device_id_for_broadcast).cloned()
                    };

                    // If sender found, send message
                    if let Some(tx) = tx {
                        let _ = tx.send(bin).await;
                    }
                }
            }
        });

        // Handle messages from device
        loop {
            // Read message length
            let mut len_buffer = [0u8; 4];
            match reader.read_exact(&mut len_buffer).await {
                Ok(_) => {
                    let len = u32::from_be_bytes(len_buffer) as usize;
                    let mut buffer = vec![0u8; len];

                    match reader.read_exact(&mut buffer).await {
                        Ok(_) => {
                            // Parse device message
                            match bincode::deserialize::<Message>(&buffer) {
                                Ok(frame) => {
                                    // Broadcast device message
                                    let _ = device_tx.send(frame);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to deserialize device message: {}", e);
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                Err(_) => break,
            }
        }

        // Clean up device connection
        {
            let mut devices_map = devices.lock().unwrap();
            devices_map.remove(&device_id_for_broadcast);
        }

        // Stop tasks
        send_task.abort();
        broadcast_task.abort();

        tracing::info!("Device {} disconnected", device_id_for_broadcast);
    }
}

#[async_trait]
impl MessageProtocol for TcpProtocol {
    fn name(&self) -> &'static str {
        "tcp"
    }

    async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let listener = TcpListener::bind(&self.addr).await?;
        tracing::info!("TCP server listening on {}", self.addr);

        // Create stop signal channel
        let (stop_tx, stop_rx) = oneshot::channel();
        {
            let mut stop_tx_guard = self.stop_tx.lock().unwrap();
            *stop_tx_guard = Some(stop_tx);
        }

        // Start server in new task
        let devices = self.devices.clone();
        let device_tx = self.device_tx.clone();
        tokio::spawn(async move {
            let mut stop_fut = Box::pin(stop_rx);

            loop {
                tokio::select! {
                    _ = &mut stop_fut => {
                        tracing::info!("TCP server shutting down");
                        break;
                    },
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                let devices_clone = devices.clone();
                                let device_tx_clone = device_tx.clone();
                                tokio::spawn(async move {
                                    Self::handle_connection(stream, addr, devices_clone, device_tx_clone).await;
                                });
                            },
                            Err(e) => {
                                tracing::error!("Failed to accept TCP connection: {}", e);
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    async fn send_app_message(
        &self,
        _message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // TCP protocol doesn't directly handle application messages
        // If needed, could convert to DeviceFrame and send
        Ok(())
    }

    async fn send_device_message(
        &self,
        message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Send device message through broadcast channel
        let _ = self.device_tx.send(message);
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Send stop signal
        let mut stop_tx_guard = self.stop_tx.lock().unwrap();
        if let Some(tx) = stop_tx_guard.take() {
            let _ = tx.send(());
        }

        // Clean up device connections
        let mut devices = self.devices.lock().unwrap();
        devices.clear();

        Ok(())
    }
}
