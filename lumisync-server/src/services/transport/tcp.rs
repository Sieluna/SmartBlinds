use embedded_io_adapters::tokio_1::FromTokio;
use lumisync_api::message::Message;
use lumisync_api::transport::{AsyncMessageTransport, Protocol};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, oneshot};

use crate::services::MessageRouter;

pub struct TcpTransport {
    addr: SocketAddr,
    message_router: Arc<MessageRouter>,
    protocol: Protocol,
    enable_crc: bool,
}

impl TcpTransport {
    pub fn new(addr: SocketAddr, message_router: Arc<MessageRouter>) -> Self {
        Self {
            addr,
            message_router,
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

    pub async fn start(
        &self,
    ) -> Result<oneshot::Sender<()>, Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(&self.addr).await?;
        tracing::info!("TCP transport listening on {}", self.addr);

        let (stop_tx, mut stop_rx) = oneshot::channel();
        let message_router = self.message_router.clone();
        let protocol = self.protocol;
        let enable_crc = self.enable_crc;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        tracing::info!("TCP transport shutting down");
                        break;
                    },
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                let message_router = message_router.clone();
                                tokio::spawn(async move {
                                    Self::handle_connection(
                                        stream,
                                        addr,
                                        message_router,
                                        protocol,
                                        enable_crc
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

        Ok(stop_tx)
    }

    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        message_router: Arc<MessageRouter>,
        protocol: Protocol,
        enable_crc: bool,
    ) {
        let embedded_io = FromTokio::new(stream);
        let transport = Arc::new(Mutex::new(
            AsyncMessageTransport::new(embedded_io)
                .with_default_protocol(protocol)
                .with_crc(enable_crc),
        ));

        let device_id: i32 = match transport.lock().await.receive_message().await {
            Ok((id, _, _)) => id,
            Err(e) => {
                tracing::error!("Handshake failed from {}: {:?}", addr, e);
                return;
            }
        };

        tracing::info!("Device {} connected from {}", device_id, addr);

        let (subscription_id, mut device_messages) =
            message_router.subscribe_device_messages().await;

        let transport_send = transport.clone();
        let send_task = tokio::spawn(async move {
            while let Some(message) = device_messages.recv().await {
                if transport_send
                    .lock()
                    .await
                    .send_message(&message, None, None)
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        loop {
            match transport.lock().await.receive_message::<Message>().await {
                Ok((message, _, _)) => {
                    message_router.process_incoming_message(message).await;
                }
                Err(e) => {
                    tracing::warn!("Failed to receive from device {}: {:?}", device_id, e);
                    break;
                }
            }
        }

        send_task.abort();
        message_router.unsubscribe(&subscription_id).await;
        tracing::info!("Device {} disconnected", device_id);
    }
}
