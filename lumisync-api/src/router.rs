use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::adapter::{AdapterError, AdapterManager, TransportAdapter};
use crate::handler::{HandlerWrapper, MessageError, MessageHandler, PayloadType};
use crate::message::{Message, NodeId};

/// Message router abstraction trait, defining core interface for message routing
pub trait MessageRouter: Send + Sync {
    /// Register message handler
    fn register_handler(&mut self, handler: Box<dyn MessageHandler>) -> Result<u32, RouterError>;

    /// Unregister message handler
    fn unregister_handler(&mut self, handler_id: u32) -> Result<(), RouterError>;

    /// Route message to appropriate handler
    fn route_message(&mut self, message: Message) -> Result<(), RouterError>;

    /// Send message to specified node
    fn send_message(&mut self, target: NodeId, message: Message) -> Result<(), RouterError>;

    /// Get routing table statistics
    fn get_stats(&self) -> RouterStats;

    /// Get router configuration
    fn config(&self) -> &RouterConfig;

    /// Start router service
    fn start(&mut self) -> Result<(), RouterError>;

    /// Stop router service
    fn stop(&mut self) -> Result<(), RouterError>;
}

/// Base message router implementation
pub struct BaseMessageRouter {
    /// Message handler collection
    handlers: BTreeMap<u32, HandlerWrapper>,
    /// Next handler ID
    next_handler_id: u32,
    /// Payload type to handler mapping
    payload_handlers: BTreeMap<PayloadType, Vec<u32>>,
    /// Transport adapter manager
    adapter_manager: AdapterManager,
    /// Router configuration
    config: RouterConfig,
    /// Router statistics
    stats: RouterStats,
    /// Router running status
    is_running: bool,
}

impl BaseMessageRouter {
    /// Create new message router
    pub fn new(config: RouterConfig) -> Self {
        Self {
            handlers: BTreeMap::new(),
            next_handler_id: 1,
            payload_handlers: BTreeMap::new(),
            adapter_manager: AdapterManager::new(),
            config,
            stats: RouterStats::default(),
            is_running: false,
        }
    }

    /// Get mutable reference to transport adapter manager
    pub fn adapter_manager_mut(&mut self) -> &mut AdapterManager {
        &mut self.adapter_manager
    }

    /// Find handlers based on message payload type
    fn find_handlers_for_payload(&self, payload_type: PayloadType) -> Vec<u32> {
        self.payload_handlers
            .get(&payload_type)
            .cloned()
            .unwrap_or_default()
    }

    /// Update payload type mapping
    fn update_payload_mapping(&mut self, handler_id: u32, payload_types: Vec<PayloadType>) {
        for payload_type in payload_types {
            self.payload_handlers
                .entry(payload_type)
                .or_insert_with(Vec::new)
                .push(handler_id);
        }
    }

    /// Remove payload type mapping
    fn remove_payload_mapping(&mut self, handler_id: u32) {
        for handlers in self.payload_handlers.values_mut() {
            handlers.retain(|&id| id != handler_id);
        }
    }
}

impl MessageRouter for BaseMessageRouter {
    fn register_handler(&mut self, handler: Box<dyn MessageHandler>) -> Result<u32, RouterError> {
        let handler_id = self.next_handler_id;
        self.next_handler_id += 1;

        let payload_types = handler.supported_payloads();
        let wrapper = HandlerWrapper::new(handler, handler_id);

        self.handlers.insert(handler_id, wrapper);
        self.update_payload_mapping(handler_id, payload_types);

        self.stats.registered_handlers += 1;

        Ok(handler_id)
    }

    fn unregister_handler(&mut self, handler_id: u32) -> Result<(), RouterError> {
        if self.handlers.remove(&handler_id).is_some() {
            self.remove_payload_mapping(handler_id);
            self.stats.registered_handlers = self.stats.registered_handlers.saturating_sub(1);
            Ok(())
        } else {
            Err(RouterError::HandlerNotFound(handler_id))
        }
    }

    fn route_message(&mut self, message: Message) -> Result<(), RouterError> {
        self.stats.total_messages += 1;

        let payload_type = crate::handler::PayloadType::from_payload(&message.payload);
        let handler_ids = self.find_handlers_for_payload(payload_type);

        if handler_ids.is_empty() {
            self.stats.unrouted_messages += 1;
            return Err(RouterError::NoHandlerFound(payload_type));
        }

        let mut handled = false;
        let mut last_error = None;

        for handler_id in handler_ids {
            if let Some(handler) = self.handlers.get_mut(&handler_id) {
                match handler.handle_message_with_stats(message.clone()) {
                    Ok(response_opt) => {
                        handled = true;
                        self.stats.routed_messages += 1;

                        // If there's a response message, send it back to source node
                        if let Some(response) = response_opt {
                            let _ = self.send_message(message.header.source, response);
                        }
                        break; // Stop at first successful handler
                    }
                    Err(e) => {
                        last_error = Some(e);
                        continue; // Try next handler
                    }
                }
            }
        }

        if !handled {
            self.stats.failed_messages += 1;
            if let Some(error) = last_error {
                return Err(RouterError::HandlingFailed(error));
            } else {
                return Err(RouterError::NoHandlerFound(payload_type));
            }
        }

        Ok(())
    }

    fn send_message(&mut self, target: NodeId, message: Message) -> Result<(), RouterError> {
        self.adapter_manager
            .send_to(target, &message)
            .map_err(RouterError::TransportError)
    }

    fn get_stats(&self) -> RouterStats {
        self.stats.clone()
    }

    fn config(&self) -> &RouterConfig {
        &self.config
    }

    fn start(&mut self) -> Result<(), RouterError> {
        if self.is_running {
            return Err(RouterError::AlreadyRunning);
        }

        self.is_running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), RouterError> {
        if !self.is_running {
            return Err(RouterError::NotRunning);
        }

        self.is_running = false;
        Ok(())
    }
}

/// Router configuration
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Maximum number of handlers
    pub max_handlers: usize,
    /// Message processing timeout (milliseconds)
    pub message_timeout_ms: u64,
    /// Enable duplicate message detection
    pub enable_duplicate_detection: bool,
    /// Duplicate message detection window size
    pub duplicate_window_size: usize,
    /// Enable message statistics
    pub enable_stats: bool,
    /// Statistics data retention time (milliseconds)
    pub stats_retention_ms: u64,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            max_handlers: 64,
            message_timeout_ms: 5000,
            enable_duplicate_detection: true,
            duplicate_window_size: 1000,
            enable_stats: true,
            stats_retention_ms: 3600000, // 1 hour
        }
    }
}

/// Router statistics
#[derive(Debug, Clone, Default)]
pub struct RouterStats {
    /// Total number of processed messages
    pub total_messages: u64,
    /// Number of successfully routed messages
    pub routed_messages: u64,
    /// Number of unrouted messages
    pub unrouted_messages: u64,
    /// Number of failed messages
    pub failed_messages: u64,
    /// Number of registered handlers
    pub registered_handlers: usize,
    /// Average message processing time (milliseconds)
    pub average_processing_time_ms: f64,
    /// Maximum message processing time (milliseconds)
    pub max_processing_time_ms: u64,
    /// Minimum message processing time (milliseconds)
    pub min_processing_time_ms: u64,
}

impl RouterStats {
    /// Calculate message processing success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_messages == 0 {
            0.0
        } else {
            self.routed_messages as f64 / self.total_messages as f64
        }
    }

    /// Calculate average handler load
    pub fn average_handler_load(&self) -> f64 {
        if self.registered_handlers == 0 {
            0.0
        } else {
            self.routed_messages as f64 / self.registered_handlers as f64
        }
    }
}

/// Router error types
#[derive(Debug, Clone, PartialEq)]
pub enum RouterError {
    /// Handler not found
    HandlerNotFound(u32),
    /// No suitable handler found
    NoHandlerFound(PayloadType),
    /// Message handling failed
    HandlingFailed(MessageError),
    /// Transport error
    TransportError(AdapterError),
    /// Router already running
    AlreadyRunning,
    /// Router not running
    NotRunning,
    /// Configuration error
    ConfigError(String),
    /// Exceeded maximum number of handlers
    TooManyHandlers,
    /// Timeout
    Timeout,
    /// Duplicate message
    DuplicateMessage,
}

impl core::fmt::Display for RouterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RouterError::HandlerNotFound(id) => write!(f, "Handler not found: {}", id),
            RouterError::NoHandlerFound(payload_type) => {
                write!(f, "No handler found for payload type: {:?}", payload_type)
            }
            RouterError::HandlingFailed(error) => write!(f, "Message handling failed: {}", error),
            RouterError::TransportError(error) => write!(f, "Transport error: {}", error),
            RouterError::AlreadyRunning => write!(f, "Router is already running"),
            RouterError::NotRunning => write!(f, "Router is not running"),
            RouterError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            RouterError::TooManyHandlers => write!(f, "Too many handlers registered"),
            RouterError::Timeout => write!(f, "Operation timed out"),
            RouterError::DuplicateMessage => write!(f, "Duplicate message detected"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RouterError {}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::handler::{MessageHandler, PayloadType};
    use crate::message::{AckPayload, MessageHeader, MessagePayload, Priority};

    use super::*;

    struct TestMessageHandler {
        node_id: NodeId,
        name: &'static str,
        supported_types: Vec<PayloadType>,
        response_message: Option<Message>,
    }

    impl TestMessageHandler {
        fn new(
            node_id: NodeId,
            name: &'static str,
            supported_types: Vec<PayloadType>,
            response_message: Option<Message>,
        ) -> Self {
            Self {
                node_id,
                name,
                supported_types,
                response_message,
            }
        }
    }

    impl MessageHandler for TestMessageHandler {
        fn handle_message(&mut self, _message: Message) -> Result<Option<Message>, MessageError> {
            Ok(self.response_message.clone())
        }

        fn supported_payloads(&self) -> Vec<PayloadType> {
            self.supported_types.clone()
        }

        fn node_id(&self) -> NodeId {
            self.node_id
        }

        fn name(&self) -> &'static str {
            self.name
        }
    }

    #[test]
    fn test_router_handler_registration() {
        let config = RouterConfig::default();
        let mut router = BaseMessageRouter::new(config);

        let handler = TestMessageHandler::new(
            NodeId::Cloud,
            "test_handler",
            vec![PayloadType::CloudCommand],
            None,
        );

        let handler_id = router.register_handler(Box::new(handler)).unwrap();

        assert_eq!(handler_id, 1);
        assert_eq!(router.handlers.len(), 1);

        // Unregister handler
        router.unregister_handler(handler_id).unwrap();
        assert_eq!(router.handlers.len(), 0);
    }

    #[test]
    fn test_message_routing() {
        let config = RouterConfig::default();
        let mut router = BaseMessageRouter::new(config);

        let response_message = Message {
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

        let handler = TestMessageHandler::new(
            NodeId::Cloud,
            "ack_handler",
            vec![PayloadType::Acknowledge],
            Some(response_message.clone()),
        );

        router.register_handler(Box::new(handler)).unwrap();

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

        let result = router.route_message(test_message);
        assert!(result.is_ok());

        let stats = router.get_stats();
        assert_eq!(stats.total_messages, 1);
        assert_eq!(stats.routed_messages, 1);
    }

    #[test]
    fn test_no_handler_found() {
        let config = RouterConfig::default();
        let mut router = BaseMessageRouter::new(config);

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

        let result = router.route_message(test_message);
        assert!(matches!(result, Err(RouterError::NoHandlerFound(_))));

        let stats = router.get_stats();
        assert_eq!(stats.total_messages, 1);
        assert_eq!(stats.unrouted_messages, 1);
    }

    #[test]
    fn test_router_stats() {
        let mut stats = RouterStats::default();
        stats.total_messages = 100;
        stats.routed_messages = 95;
        stats.registered_handlers = 5;

        assert_eq!(stats.success_rate(), 0.95);
        assert_eq!(stats.average_handler_load(), 19.0);
    }
}
