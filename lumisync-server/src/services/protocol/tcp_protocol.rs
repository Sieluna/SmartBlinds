use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use embedded_io_adapters::tokio_1::FromTokio;
use lumisync_api::{
    Message,
    transport::{AsyncMessageTransport, Protocol},
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast, mpsc, oneshot};

use super::MessageProtocol;

const CHANNEL_CAPACITY: usize = 1000;

#[derive(Debug)]
pub struct TcpProtocol {
    addr: SocketAddr,
    devices: Arc<RwLock<HashMap<i32, mpsc::Sender<Message>>>>,
    device_tx: Arc<broadcast::Sender<Message>>,
    stop_tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
    protocol: Protocol,
    enable_crc: bool,
}

impl TcpProtocol {
    pub fn new(addr: SocketAddr) -> Self {
        let (device_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            addr,
            devices: Arc::new(RwLock::new(HashMap::new())),
            device_tx: Arc::new(device_tx),
            stop_tx: Arc::new(tokio::sync::Mutex::new(None)),
            protocol: Protocol::Postcard,
            enable_crc: true,
        }
    }

    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn with_crc(mut self, enable: bool) -> Self {
        self.enable_crc = enable;
        self
    }

    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        devices: Arc<RwLock<HashMap<i32, mpsc::Sender<Message>>>>,
        device_tx: Arc<broadcast::Sender<Message>>,
        protocol: Protocol,
        enable_crc: bool,
    ) {
        let peer_addr = addr;

        // Convert Tokio stream to embedded-io compatible
        let embedded_io = FromTokio::new(stream);
        let mut transport = AsyncMessageTransport::new(embedded_io)
            .with_default_protocol(protocol)
            .with_crc(enable_crc);

        // Perform handshake - expect device ID as first message
        let device_id: i32 = match transport.receive_message().await {
            Ok((id, _, _)) => id,
            Err(e) => {
                tracing::error!("Handshake failed from {}: {:?}", peer_addr, e);
                return;
            }
        };

        tracing::info!(
            "Device {} connected from {} using protocol {:?} (CRC: {})",
            device_id,
            peer_addr,
            protocol,
            enable_crc
        );

        // Create channels for message coordination
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);
        let (send_cmd_tx, mut send_cmd_rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);

        // Register device
        {
            let mut devices_map = devices.write().await;
            if devices_map.contains_key(&device_id) {
                tracing::warn!(
                    "Device {} already connected, replacing existing connection",
                    device_id
                );
            }
            devices_map.insert(device_id, outgoing_tx.clone());
        }

        // Subscribe to broadcast messages
        let mut broadcast_rx = device_tx.subscribe();

        // Task to merge outgoing and broadcast messages
        let send_cmd_tx_clone = send_cmd_tx.clone();
        let message_merge_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Direct outgoing messages
                    msg_opt = outgoing_rx.recv() => {
                        match msg_opt {
                            Some(msg) => {
                                if let Err(_) = send_cmd_tx_clone.send(msg).await {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    // Broadcast messages
                    broadcast_result = broadcast_rx.recv() => {
                        match broadcast_result {
                            Ok(msg) => {
                                if let Err(_) = send_cmd_tx_clone.send(msg).await {
                                    break;
                                }
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                tracing::warn!("Device {} lagged behind broadcast messages", device_id);
                                continue;
                            }
                        }
                    }
                }
            }
        });

        // Main communication loop
        let devices_for_cleanup = devices.clone();
        let device_tx_for_send = device_tx.clone();

        loop {
            tokio::select! {
                // Handle incoming messages from device
                receive_result = transport.receive_message::<Message>() => {
                    match receive_result {
                        Ok((message, received_protocol, stream_id)) => {
                            tracing::debug!(
                                "Received message from device {} using protocol {:?}, stream_id: {:?}",
                                device_id, received_protocol, stream_id
                            );

                            if let Err(e) = device_tx_for_send.send(message) {
                                tracing::warn!(
                                    "Failed to broadcast message from device {}: {:?}",
                                    device_id, e
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to receive message from device {} at {}: {:?}",
                                device_id, peer_addr, e
                            );
                            break;
                        }
                    }
                }
                // Handle outgoing messages to device
                send_msg_opt = send_cmd_rx.recv() => {
                    match send_msg_opt {
                        Some(message) => {
                            if let Err(e) = transport.send_message(&message, None, None).await {
                                tracing::warn!(
                                    "Failed to send message to device {} at {}: {:?}",
                                    device_id, peer_addr, e
                                );
                                break;
                            }
                        }
                        None => {
                            tracing::debug!("Send command channel closed for device {}", device_id);
                            break;
                        }
                    }
                }
            }
        }

        // Cleanup
        message_merge_task.abort();

        {
            let mut devices_map = devices_for_cleanup.write().await;
            devices_map.remove(&device_id);
        }

        tracing::info!("Device {} at {} disconnected", device_id, peer_addr);
    }
}

#[async_trait]
impl MessageProtocol for TcpProtocol {
    fn name(&self) -> &'static str {
        "tcp-framed"
    }

    async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let listener = TcpListener::bind(&self.addr).await.map_err(|e| {
            tracing::error!("Failed to bind TCP listener to {}: {}", self.addr, e);
            Box::new(e) as Box<dyn Error + Send + Sync>
        })?;

        tracing::info!(
            "TCP framed server listening on {} with protocol {:?}, CRC: {}",
            self.addr,
            self.protocol,
            self.enable_crc
        );

        let (stop_tx, stop_rx) = oneshot::channel();
        {
            let mut stop_tx_guard = self.stop_tx.lock().await;
            *stop_tx_guard = Some(stop_tx);
        }

        let devices = self.devices.clone();
        let device_tx = self.device_tx.clone();
        let protocol = self.protocol;
        let enable_crc = self.enable_crc;

        tokio::spawn(async move {
            let mut stop_fut = Box::pin(stop_rx);

            loop {
                tokio::select! {
                    _ = &mut stop_fut => {
                        tracing::info!("TCP framed server shutting down");
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

                                tokio::spawn(async move {
                                    Self::handle_connection(
                                        stream,
                                        addr,
                                        devices_clone,
                                        device_tx_clone,
                                        protocol,
                                        enable_crc,
                                    ).await;
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
        tracing::debug!("TCP framed protocol doesn't handle app messages directly");
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
        tracing::info!("Stopping TCP framed protocol server");

        let mut stop_tx_guard = self.stop_tx.lock().await;
        if let Some(tx) = stop_tx_guard.take() {
            if let Err(e) = tx.send(()) {
                tracing::warn!("Failed to send stop signal: {:?}", e);
            }
        }

        let mut devices = self.devices.write().await;
        let count = devices.len();
        devices.clear();
        tracing::info!("Closed {} device connections", count);

        Ok(())
    }
}
