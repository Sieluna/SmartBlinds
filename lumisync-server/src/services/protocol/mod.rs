mod tcp_protocol;
mod ws_protocol;

pub use tcp_protocol::*;
pub use ws_protocol::*;

use async_trait::async_trait;
use lumisync_api::Message;
use std::error::Error;
use std::fmt::Debug;

/// Defines the basic traits for protocol adapters
#[async_trait]
pub trait MessageProtocol: Send + Sync + Debug {
    /// Protocol name
    fn name(&self) -> &'static str;

    /// Start the protocol server
    async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Send application message
    async fn send_app_message(&self, message: Message) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Send device message
    async fn send_device_message(
        &self,
        message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Stop the protocol server
    async fn stop(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
}

/// Protocol type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolType {
    WebSocket,
    Tcp,
}

impl ProtocolType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProtocolType::WebSocket => "websocket",
            ProtocolType::Tcp => "tcp",
        }
    }
}

pub struct ProtocolManager {
    pub protocols: Vec<Box<dyn MessageProtocol>>,
}

impl ProtocolManager {
    pub fn new() -> Self {
        Self {
            protocols: Vec::new(),
        }
    }

    pub fn add_protocol(&mut self, protocol: Box<dyn MessageProtocol>) {
        self.protocols.push(protocol);
    }

    pub async fn start_all(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        for protocol in &self.protocols {
            protocol.start().await?;
        }
        Ok(())
    }

    pub async fn stop_all(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        for protocol in &self.protocols {
            protocol.stop().await?;
        }
        Ok(())
    }

    pub async fn broadcast_app_message(
        &self,
        message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for protocol in &self.protocols {
            if let Err(e) = protocol.send_app_message(message.clone()).await {
                tracing::warn!("Failed to send message through {}: {}", protocol.name(), e);
            }
        }
        Ok(())
    }

    pub async fn broadcast_device_message(
        &self,
        message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for protocol in &self.protocols {
            if let Err(e) = protocol.send_device_message(message.clone()).await {
                tracing::warn!(
                    "Failed to send device message through {}: {}",
                    protocol.name(),
                    e
                );
            }
        }
        Ok(())
    }
}

impl Default for ProtocolManager {
    fn default() -> Self {
        Self::new()
    }
}
