use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::message::{Message, NodeId};

/// Transport adapter abstraction trait, defining unified interface for underlying transport
pub trait TransportAdapter: Send + Sync {
    type Error: core::fmt::Debug + Send + Sync;

    /// Send message to specified node
    fn send_to(&mut self, target: NodeId, message: &Message) -> Result<(), Self::Error>;

    /// Try to receive message from any node (non-blocking)
    fn try_receive(&mut self) -> Result<Option<(NodeId, Message)>, Self::Error>;

    /// Check connection status with specific node
    fn is_connected(&self, node: NodeId) -> bool;

    /// Get all connected nodes
    fn connected_nodes(&self) -> Vec<NodeId>;

    /// Try to connect to specified node
    fn connect(&mut self, target: NodeId) -> Result<(), Self::Error>;

    /// Disconnect from specified node
    fn disconnect(&mut self, target: NodeId) -> Result<(), Self::Error>;

    /// Get transport type of the adapter
    fn transport_type(&self) -> TransportType;

    /// Get configuration information of the adapter
    fn config(&self) -> &TransportConfig;

    /// Get transport statistics
    fn stats(&self) -> TransportStats;
}

/// Transport type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransportType {
    /// TCP transport (Cloud-Edge communication)
    Tcp,
    /// UDP transport (local broadcast)
    Udp,
    /// BLE transport (Edge-Device communication)
    Ble,
    /// WebSocket transport (Web application communication)
    WebSocket,
    /// Mock transport for testing
    Mock,
}

/// Transport configuration
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Maximum number of connections
    pub max_connections: usize,
    /// Connection timeout (milliseconds)
    pub connect_timeout_ms: u64,
    /// Message send timeout (milliseconds)
    pub send_timeout_ms: u64,
    /// Receive buffer size
    pub receive_buffer_size: usize,
    /// Send buffer size
    pub send_buffer_size: usize,
    /// Enable CRC checksum
    pub enable_crc: bool,
    /// Number of retries
    pub max_retries: u8,
    /// Heartbeat interval (milliseconds)
    pub heartbeat_interval_ms: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_connections: 32,
            connect_timeout_ms: 5000,
            send_timeout_ms: 3000,
            receive_buffer_size: 4096,
            send_buffer_size: 4096,
            enable_crc: true,
            max_retries: 3,
            heartbeat_interval_ms: 30000,
        }
    }
}

/// Transport statistics
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    /// Number of messages sent
    pub messages_sent: u64,
    /// Number of messages received
    pub messages_received: u64,
    /// Number of send failures
    pub send_failures: u64,
    /// Number of receive failures
    pub receive_failures: u64,
    /// Current number of connections
    pub active_connections: usize,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Average latency (milliseconds)
    pub average_latency_ms: f64,
    /// Number of connections established
    pub connections_established: u64,
    /// Number of connection failures
    pub connection_failures: u64,
}

impl TransportStats {
    /// Calculate message success rate
    pub fn message_success_rate(&self) -> f64 {
        let total_attempts = self.messages_sent + self.send_failures;
        if total_attempts == 0 {
            0.0
        } else {
            self.messages_sent as f64 / total_attempts as f64
        }
    }

    /// Calculate connection success rate
    pub fn connection_success_rate(&self) -> f64 {
        let total_attempts = self.connections_established + self.connection_failures;
        if total_attempts == 0 {
            0.0
        } else {
            self.connections_established as f64 / total_attempts as f64
        }
    }
}

/// Transport adapter error types
#[derive(Debug, Clone, PartialEq)]
pub enum AdapterError {
    /// Connection failed
    ConnectionFailed(String),
    /// Send failed
    SendFailed(String),
    /// Receive failed
    ReceiveFailed(String),
    /// Serialization failed
    SerializationFailed(String),
    /// Node not connected
    NodeNotConnected(NodeId),
    /// Buffer full
    BufferFull,
    /// Timeout
    Timeout,
    /// Configuration error
    ConfigError(String),
    /// Unsupported operation
    UnsupportedOperation,
    /// Network error
    NetworkError(String),
}

impl core::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AdapterError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            AdapterError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            AdapterError::ReceiveFailed(msg) => write!(f, "Receive failed: {}", msg),
            AdapterError::SerializationFailed(msg) => write!(f, "Serialization failed: {}", msg),
            AdapterError::NodeNotConnected(node) => write!(f, "Node not connected: {:?}", node),
            AdapterError::BufferFull => write!(f, "Buffer is full"),
            AdapterError::Timeout => write!(f, "Operation timed out"),
            AdapterError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            AdapterError::UnsupportedOperation => write!(f, "Unsupported operation"),
            AdapterError::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AdapterError {}

/// Transport adapter manager
pub struct AdapterManager {
    // Simplified implementation, using Vec instead of dyn trait objects
    tcp_adapters: Vec<Box<dyn TransportAdapter<Error = AdapterError>>>,
    ble_adapters: Vec<Box<dyn TransportAdapter<Error = AdapterError>>>,
    routing_table: BTreeMap<NodeId, TransportType>,
}

impl AdapterManager {
    /// Create new adapter manager
    pub fn new() -> Self {
        Self {
            tcp_adapters: Vec::new(),
            ble_adapters: Vec::new(),
            routing_table: BTreeMap::new(),
        }
    }

    /// Register transport adapter
    pub fn register_adapter(
        &mut self,
        transport_type: TransportType,
        adapter: Box<dyn TransportAdapter<Error = AdapterError>>,
    ) {
        match transport_type {
            TransportType::Tcp | TransportType::WebSocket => {
                self.tcp_adapters.push(adapter);
            }
            TransportType::Ble | TransportType::Udp => {
                self.ble_adapters.push(adapter);
            }
            _ => {
                // Other types temporarily go to TCP group
                self.tcp_adapters.push(adapter);
            }
        }
    }

    /// Set node routing
    pub fn set_route(&mut self, node: NodeId, transport_type: TransportType) {
        self.routing_table.insert(node, transport_type);
    }

    /// Send message to specified node
    pub fn send_to(&mut self, target: NodeId, message: &Message) -> Result<(), AdapterError> {
        let transport_type = self
            .routing_table
            .get(&target)
            .or_else(|| self.default_transport_for_node(&target))
            .ok_or_else(|| AdapterError::NodeNotConnected(target))?;

        let adapters = match transport_type {
            TransportType::Tcp | TransportType::WebSocket => &mut self.tcp_adapters,
            TransportType::Ble | TransportType::Udp => &mut self.ble_adapters,
            _ => &mut self.tcp_adapters,
        };

        // Try to send with the first available adapter
        for adapter in adapters.iter_mut() {
            if adapter.is_connected(target) {
                return adapter.send_to(target, message);
            }
        }

        Err(AdapterError::NodeNotConnected(target))
    }

    /// Receive message from any adapter
    pub fn try_receive_any(&mut self) -> Result<Option<(NodeId, Message)>, AdapterError> {
        // Check TCP adapters first
        for adapter in &mut self.tcp_adapters {
            if let Ok(Some(message)) = adapter.try_receive() {
                return Ok(Some(message));
            }
        }

        // Then check BLE adapters
        for adapter in &mut self.ble_adapters {
            if let Ok(Some(message)) = adapter.try_receive() {
                return Ok(Some(message));
            }
        }

        Ok(None)
    }

    /// Infer default transport method based on node type
    fn default_transport_for_node(&self, node: &NodeId) -> Option<&TransportType> {
        match node {
            NodeId::Cloud => Some(&TransportType::Tcp),
            NodeId::Edge(_) => Some(&TransportType::Tcp),
            NodeId::Device(_) => Some(&TransportType::Ble),
            NodeId::Any => None,
        }
    }

    /// Get all transport statistics
    pub fn get_all_stats(&self) -> BTreeMap<TransportType, TransportStats> {
        let mut stats = BTreeMap::new();

        // Collect TCP adapter statistics
        for adapter in &self.tcp_adapters {
            let transport_type = adapter.transport_type();
            stats.insert(transport_type, adapter.stats());
        }

        // Collect BLE adapter statistics
        for adapter in &self.ble_adapters {
            let transport_type = adapter.transport_type();
            stats.insert(transport_type, adapter.stats());
        }

        stats
    }
}

impl Default for AdapterManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::message::{AckPayload, MessageHeader, MessagePayload, Priority};

    use super::*;

    struct MockAdapter {
        transport_type: TransportType,
        config: TransportConfig,
        stats: TransportStats,
        connected_nodes: Vec<NodeId>,
        message_queue: Vec<(NodeId, Message)>,
    }

    impl MockAdapter {
        fn new(transport_type: TransportType) -> Self {
            Self {
                transport_type,
                config: TransportConfig::default(),
                stats: TransportStats::default(),
                connected_nodes: Vec::new(),
                message_queue: Vec::new(),
            }
        }
    }

    impl TransportAdapter for MockAdapter {
        type Error = AdapterError;

        fn send_to(&mut self, _target: NodeId, _message: &Message) -> Result<(), Self::Error> {
            self.stats.messages_sent += 1;
            Ok(())
        }

        fn try_receive(&mut self) -> Result<Option<(NodeId, Message)>, Self::Error> {
            if !self.message_queue.is_empty() {
                self.stats.messages_received += 1;

                let test_message = Message {
                    header: MessageHeader {
                        id: Uuid::new_v4(),
                        timestamp: OffsetDateTime::now_utc(),
                        priority: Priority::Regular,
                        source: NodeId::Cloud,
                        target: NodeId::Edge(1),
                    },
                    payload: MessagePayload::Acknowledge(AckPayload {
                        original_msg_id: Uuid::new_v4(),
                        status: "OK".into(),
                        details: None,
                    }),
                };

                Ok(Some((NodeId::Cloud, test_message)))
            } else {
                Ok(None)
            }
        }

        fn is_connected(&self, node: NodeId) -> bool {
            self.connected_nodes.contains(&node)
        }

        fn connected_nodes(&self) -> Vec<NodeId> {
            self.connected_nodes.clone()
        }

        fn connect(&mut self, target: NodeId) -> Result<(), Self::Error> {
            if !self.connected_nodes.contains(&target) {
                self.connected_nodes.push(target);
                self.stats.connections_established += 1;
            }
            Ok(())
        }

        fn disconnect(&mut self, target: NodeId) -> Result<(), Self::Error> {
            self.connected_nodes.retain(|&node| node != target);
            Ok(())
        }

        fn transport_type(&self) -> TransportType {
            self.transport_type
        }

        fn config(&self) -> &TransportConfig {
            &self.config
        }

        fn stats(&self) -> TransportStats {
            self.stats.clone()
        }
    }

    #[test]
    fn test_mock_adapter() {
        let mut adapter = MockAdapter::new(TransportType::Mock);

        // Test connection
        let test_node = NodeId::Edge(1);
        assert!(!adapter.is_connected(test_node));

        adapter.connect(test_node).unwrap();
        assert!(adapter.is_connected(test_node));

        // Test sending
        let test_message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: test_node,
            },
            payload: MessagePayload::Acknowledge(AckPayload {
                original_msg_id: Uuid::new_v4(),
                status: "Test".into(),
                details: None,
            }),
        };

        adapter.send_to(test_node, &test_message).unwrap();
        assert_eq!(adapter.stats().messages_sent, 1);

        // Add message to queue for testing receive
        adapter.message_queue.push((NodeId::Cloud, test_message));

        // Test receiving
        let result = adapter.try_receive().unwrap();
        assert!(result.is_some());
        assert_eq!(adapter.stats().messages_received, 1);
    }

    #[test]
    fn test_adapter_manager() {
        let mut manager = AdapterManager::new();

        // Register adapters
        let mut tcp_adapter = MockAdapter::new(TransportType::Tcp);
        tcp_adapter.connect(NodeId::Cloud).unwrap(); // Pre-connect
        let mut ble_adapter = MockAdapter::new(TransportType::Ble);
        ble_adapter
            .connect(NodeId::Device([1, 2, 3, 4, 5, 6]))
            .unwrap(); // Pre-connect

        manager.register_adapter(TransportType::Tcp, Box::new(tcp_adapter));
        manager.register_adapter(TransportType::Ble, Box::new(ble_adapter));

        // Set routing
        manager.set_route(NodeId::Cloud, TransportType::Tcp);
        manager.set_route(NodeId::Device([1, 2, 3, 4, 5, 6]), TransportType::Ble);

        // Test routing selection
        let test_message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::Acknowledge(AckPayload {
                original_msg_id: Uuid::new_v4(),
                status: "Test".into(),
                details: None,
            }),
        };

        let result = manager.send_to(NodeId::Cloud, &test_message);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transport_stats() {
        let mut stats = TransportStats::default();
        stats.messages_sent = 95;
        stats.send_failures = 5;
        stats.connections_established = 10;
        stats.connection_failures = 2;

        assert_eq!(stats.message_success_rate(), 0.95);
        assert!((stats.connection_success_rate() - 0.8333333333333334).abs() < 0.001);
    }
}
