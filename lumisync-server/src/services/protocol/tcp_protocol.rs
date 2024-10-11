use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use lumisync_api::{Message, Protocol, SerializationProtocol};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, oneshot};

use super::MessageProtocol;

const MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB
const CHANNEL_CAPACITY: usize = 1000;
const BUFFER_SIZE: usize = 8 * 1024; // 8KB

#[derive(Debug)]
pub struct TcpProtocol {
    /// TCP server address
    addr: SocketAddr,
    /// Device connection management
    devices: Arc<RwLock<HashMap<i32, mpsc::Sender<Vec<u8>>>>>,
    /// Device message broadcast channel
    device_tx: Arc<broadcast::Sender<Message>>,
    /// Stop server sender
    stop_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    /// Serialization protocol
    protocol: SerializationProtocol,
}

impl TcpProtocol {
    pub fn new(addr: SocketAddr) -> Self {
        let (device_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            addr,
            devices: Arc::new(RwLock::new(HashMap::new())),
            device_tx: Arc::new(device_tx),
            stop_tx: Arc::new(Mutex::new(None)),
            protocol: Default::default(),
        }
    }

    pub fn with_protocol(mut self, protocol: SerializationProtocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn protocol(&self) -> &SerializationProtocol {
        &self.protocol
    }

    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        devices: Arc<RwLock<HashMap<i32, mpsc::Sender<Vec<u8>>>>>,
        device_tx: Arc<broadcast::Sender<Message>>,
        protocol: SerializationProtocol,
    ) {
        let (reader_half, writer_half) = stream.into_split();
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, reader_half);
        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, writer_half);

        // Read device ID
        let mut id_buffer = [0u8; 4];
        if let Err(e) = reader.read_exact(&mut id_buffer).await {
            tracing::error!("Failed to read device ID from {}: {}", addr, e);
            return;
        }

        let device_id = i32::from_be_bytes(id_buffer);
        tracing::info!("Device {} connected from {}", device_id, addr);

        // Create send channel with increased capacity
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(CHANNEL_CAPACITY);

        // Register device
        {
            let mut devices_map = devices.write().unwrap();
            if devices_map.contains_key(&device_id) {
                tracing::warn!(
                    "Device {} already connected, replacing existing connection",
                    device_id
                );
            }
            devices_map.insert(device_id, tx);
        }

        // Subscribe to device message broadcast
        let mut device_rx = device_tx.subscribe();

        let send_task = tokio::spawn(async move {
            // Pre-allocate write buffer
            let mut buffer = Vec::with_capacity(BUFFER_SIZE);

            while let Some(data) = rx.recv().await {
                buffer.clear();

                // Prepare message with length prefix
                let len = data.len() as u32;
                buffer.extend_from_slice(&len.to_be_bytes());
                buffer.extend_from_slice(&data);

                // Write entire message at once to reduce system calls
                if let Err(e) = writer.write_all(&buffer).await {
                    tracing::warn!("Failed to write data to device {}: {}", device_id, e);
                    break;
                }

                // Flush buffer to ensure data is sent
                if let Err(e) = writer.flush().await {
                    tracing::warn!("Failed to flush data to device {}: {}", device_id, e);
                    break;
                }
            }
        });

        // Task to handle broadcast messages
        let devices_for_broadcast = devices.clone();
        let device_id_for_broadcast = device_id;
        let protocol_clone = protocol.clone();
        let broadcast_task = tokio::spawn(async move {
            while let Ok(device_frame) = device_rx.recv().await {
                // Serialize message
                match protocol_clone.serialize(&device_frame) {
                    Ok(bin) => {
                        // Use RwLock to get sender
                        let tx = {
                            let devices_guard = devices_for_broadcast.read().unwrap();
                            devices_guard.get(&device_id_for_broadcast).cloned()
                        };

                        // Send serialized message
                        if let Some(tx) = tx {
                            if let Err(e) = tx.send(bin).await {
                                tracing::warn!(
                                    "Failed to send broadcast message to device {}: {:?}",
                                    device_id_for_broadcast,
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to serialize message for device {}: {:?}",
                            device_id_for_broadcast,
                            e
                        );
                    }
                }
            }
        });

        // Pre-allocate read buffer
        let mut len_buffer = [0u8; 4];
        let mut message_buffer = Vec::with_capacity(BUFFER_SIZE);
        let protocol_for_reader = protocol.clone();

        // Handle messages from device
        loop {
            match reader.read_exact(&mut len_buffer).await {
                Ok(_) => {
                    let len = u32::from_be_bytes(len_buffer) as usize;

                    // Validate message length to prevent DoS attacks
                    if len > MAX_MESSAGE_SIZE {
                        tracing::warn!(
                            "Received oversized message ({} bytes) from device {}",
                            len,
                            device_id
                        );
                        break;
                    }

                    // Reuse buffer, avoid frequent memory allocation
                    if message_buffer.capacity() < len {
                        message_buffer.reserve(len - message_buffer.capacity());
                    }
                    message_buffer.resize(len, 0);

                    match reader.read_exact(&mut message_buffer).await {
                        Ok(_) => {
                            // Deserialize directly on pre-allocated buffer
                            match protocol_for_reader.deserialize::<Message>(&message_buffer) {
                                Ok(frame) => {
                                    // Broadcast device message
                                    if let Err(e) = device_tx.send(frame) {
                                        tracing::warn!(
                                            "Failed to broadcast message from device {}: {:?}",
                                            device_id,
                                            e
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to deserialize message from device {}: {:?}",
                                        device_id,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to read message data from device {}: {}",
                                device_id,
                                e
                            );
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Connection closed for device {}: {}", device_id, e);
                    break;
                }
            }
        }

        // Clean up device connection
        {
            let mut devices_map = devices.write().unwrap();
            devices_map.remove(&device_id);
        }

        // Stop tasks
        send_task.abort();
        broadcast_task.abort();

        tracing::info!("Device {} disconnected", device_id);
    }
}

#[async_trait]
impl MessageProtocol for TcpProtocol {
    fn name(&self) -> &'static str {
        "tcp"
    }

    async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let listener = TcpListener::bind(&self.addr).await.map_err(|e| {
            tracing::error!("Failed to bind TCP listener to {}: {}", self.addr, e);
            Box::new(e) as Box<dyn Error + Send + Sync>
        })?;
        tracing::info!(
            "TCP server listening on {} with {} protocol",
            self.addr,
            self.protocol.name()
        );

        let (stop_tx, stop_rx) = oneshot::channel();
        {
            let mut stop_tx_guard = self.stop_tx.lock().unwrap();
            *stop_tx_guard = Some(stop_tx);
        }

        let devices = self.devices.clone();
        let device_tx = self.device_tx.clone();
        let protocol = self.protocol.clone();
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
                                if let Err(e) = stream.set_nodelay(true) {
                                    tracing::warn!("Failed to set TCP_NODELAY: {}", e);
                                }

                                let devices_clone = devices.clone();
                                let device_tx_clone = device_tx.clone();
                                let protocol_clone = protocol.clone();
                                tokio::spawn(async move {
                                    Self::handle_connection(stream, addr, devices_clone, device_tx_clone, protocol_clone).await;
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
        tracing::debug!("TCP protocol doesn't handle app messages directly");
        Ok(())
    }

    async fn send_device_message(
        &self,
        message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Err(e) = self.device_tx.send(message) {
            tracing::warn!(
                "Failed to send device message through broadcast channel: {:?}",
                e
            );
        }
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        tracing::info!("Stopping TCP protocol server");

        let mut stop_tx_guard = self.stop_tx.lock().unwrap();
        if let Some(tx) = stop_tx_guard.take() {
            if let Err(e) = tx.send(()) {
                tracing::warn!("Failed to send stop signal: {:?}", e);
            }
        }

        let mut devices = self.devices.write().unwrap();
        let count = devices.len();
        devices.clear();
        tracing::info!("Closed {} device connections", count);

        Ok(())
    }
}
