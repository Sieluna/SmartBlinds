use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::message::{Message, MessagePayload, NodeId};

/// Message handler abstraction trait, defining core interface for processing messages
pub trait MessageHandler: Send + Sync {
    /// Process received message, return optional response message
    fn handle_message(&mut self, message: Message) -> Result<Option<Message>, MessageError>;

    /// Get message types supported by the handler
    fn supported_payloads(&self) -> Vec<PayloadType>;

    /// Get node ID of the handler
    fn node_id(&self) -> NodeId;

    /// Get name of the handler (for debugging and logging)
    fn name(&self) -> &'static str;

    /// Check if handler can process specific type of message
    fn can_handle(&self, payload_type: PayloadType) -> bool {
        self.supported_payloads().contains(&payload_type)
    }
}

/// Concrete implementation wrapper for message handler
pub struct HandlerWrapper {
    inner: Box<dyn MessageHandler>,
    handler_id: u32,
    stats: HandlerStats,
}

impl HandlerWrapper {
    pub fn new(handler: Box<dyn MessageHandler>, handler_id: u32) -> Self {
        Self {
            inner: handler,
            handler_id,
            stats: HandlerStats::default(),
        }
    }

    pub fn handler_id(&self) -> u32 {
        self.handler_id
    }

    pub fn stats(&self) -> &HandlerStats {
        &self.stats
    }

    pub fn handle_message_with_stats(
        &mut self,
        message: Message,
    ) -> Result<Option<Message>, MessageError> {
        self.stats.total_messages += 1;

        let start_time = self.get_current_time_ms();
        let result = self.inner.handle_message(message);
        let end_time = self.get_current_time_ms();

        match &result {
            Ok(_) => {
                self.stats.successful_messages += 1;
                self.stats.total_processing_time_ms += end_time - start_time;
            }
            Err(_) => {
                self.stats.failed_messages += 1;
            }
        }

        result
    }

    fn get_current_time_ms(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        }
        #[cfg(not(feature = "std"))]
        {
            // In no_std environment, time needs to be provided externally
            0
        }
    }
}

impl core::ops::Deref for HandlerWrapper {
    type Target = dyn MessageHandler;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

/// Message payload type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PayloadType {
    /// Cloud layer command
    CloudCommand,
    /// Edge layer report
    EdgeReport,
    /// Edge layer command
    EdgeCommand,
    /// Device layer report
    DeviceReport,
    /// Time synchronization message
    TimeSync,
    /// Acknowledgment message
    Acknowledge,
    /// Error message
    Error,
}

impl PayloadType {
    /// Extract type from message payload
    pub fn from_payload(payload: &MessagePayload) -> Self {
        match payload {
            MessagePayload::CloudCommand(_) => Self::CloudCommand,
            MessagePayload::EdgeReport(_) => Self::EdgeReport,
            MessagePayload::EdgeCommand(_) => Self::EdgeCommand,
            MessagePayload::DeviceReport(_) => Self::DeviceReport,
            MessagePayload::TimeSync(_) => Self::TimeSync,
            MessagePayload::Acknowledge(_) => Self::Acknowledge,
            MessagePayload::Error(_) => Self::Error,
        }
    }
}

/// Handler statistics
#[derive(Debug, Clone, Default)]
pub struct HandlerStats {
    /// Total number of processed messages
    pub total_messages: u64,
    /// Number of successfully processed messages
    pub successful_messages: u64,
    /// Number of failed messages
    pub failed_messages: u64,
    /// Total processing time (milliseconds)
    pub total_processing_time_ms: u64,
}

impl HandlerStats {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_messages == 0 {
            0.0
        } else {
            self.successful_messages as f64 / self.total_messages as f64
        }
    }

    /// Calculate average processing time
    pub fn average_processing_time_ms(&self) -> f64 {
        if self.successful_messages == 0 {
            0.0
        } else {
            self.total_processing_time_ms as f64 / self.successful_messages as f64
        }
    }
}

/// Message processing error types
#[derive(Debug, Clone, PartialEq)]
pub enum MessageError {
    /// Unauthorized operation
    Unauthorized,
    /// Invalid message format
    InvalidMessage(String),
    /// Processing timeout
    Timeout,
    /// Internal processing error
    InternalError(String),
    /// Transport layer error
    TransportError(String),
    /// Serialization/deserialization error
    SerializationError(String),
    /// Unsupported message type
    UnsupportedPayload(PayloadType),
    /// Handler busy
    HandlerBusy,
    /// Resource exhausted
    ResourceExhausted,
}

impl core::fmt::Display for MessageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MessageError::Unauthorized => write!(f, "Unauthorized operation"),
            MessageError::InvalidMessage(msg) => write!(f, "Invalid message: {}", msg),
            MessageError::Timeout => write!(f, "Message processing timeout"),
            MessageError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            MessageError::TransportError(msg) => write!(f, "Transport error: {}", msg),
            MessageError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            MessageError::UnsupportedPayload(payload_type) => {
                write!(f, "Unsupported payload type: {:?}", payload_type)
            }
            MessageError::HandlerBusy => write!(f, "Handler is busy"),
            MessageError::ResourceExhausted => write!(f, "Resource exhausted"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MessageError {}

pub type HandlerResult<T> = core::result::Result<T, MessageError>;

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::message::{MessageHeader, Priority};

    use super::*;

    struct TestHandler {
        node_id: NodeId,
        name: &'static str,
        supported_types: Vec<PayloadType>,
    }

    impl TestHandler {
        fn new(node_id: NodeId, name: &'static str, supported_types: Vec<PayloadType>) -> Self {
            Self {
                node_id,
                name,
                supported_types,
            }
        }
    }

    impl MessageHandler for TestHandler {
        fn handle_message(&mut self, _message: Message) -> Result<Option<Message>, MessageError> {
            Ok(None)
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
    fn test_payload_type_from_message() {
        use crate::message::{CloudCommand, MessagePayload};

        let cloud_command = MessagePayload::CloudCommand(CloudCommand::ControlDevices {
            commands: alloc::collections::BTreeMap::new(),
        });

        assert_eq!(
            PayloadType::from_payload(&cloud_command),
            PayloadType::CloudCommand
        );
    }

    #[test]
    fn test_handler_can_handle() {
        let handler = TestHandler::new(
            NodeId::Cloud,
            "test_handler",
            vec![PayloadType::CloudCommand, PayloadType::TimeSync],
        );

        assert!(handler.can_handle(PayloadType::CloudCommand));
        assert!(handler.can_handle(PayloadType::TimeSync));
        assert!(!handler.can_handle(PayloadType::DeviceReport));
    }

    #[test]
    fn test_handler_stats() {
        let mut stats = HandlerStats::default();
        stats.total_messages = 100;
        stats.successful_messages = 95;
        stats.failed_messages = 5;
        stats.total_processing_time_ms = 1000;

        assert_eq!(stats.success_rate(), 0.95);
        assert!((stats.average_processing_time_ms() - 10.526315789473685).abs() < 0.001);
    }

    #[test]
    fn test_handler_wrapper() {
        let test_handler = TestHandler::new(
            NodeId::Edge(1),
            "wrapper_test",
            vec![PayloadType::EdgeCommand],
        );

        let mut wrapper = HandlerWrapper::new(Box::new(test_handler), 1);

        let test_message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::EdgeCommand(crate::message::EdgeCommand::RequestHealthStatus),
        };

        let result = wrapper.handle_message_with_stats(test_message);
        assert!(result.is_ok());
        assert_eq!(wrapper.stats().total_messages, 1);
        assert_eq!(wrapper.stats().successful_messages, 1);
    }
}
