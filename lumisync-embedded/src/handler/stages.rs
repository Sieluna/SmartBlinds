use lumisync_api::{Message, MessagePayload, NodeId, Priority};
use time::OffsetDateTime;

use crate::Error;
use crate::protocol::MessageValidator;

use super::{PipelineResult, PipelineStage, ProcessContext};

pub struct ValidationStage {
    skip_timestamp_validation: bool,
}

impl ValidationStage {
    pub fn new() -> Self {
        Self {
            skip_timestamp_validation: false,
        }
    }

    pub fn skip_timestamp_validation(mut self) -> Self {
        self.skip_timestamp_validation = true;
        self
    }
}

#[async_trait::async_trait]
impl PipelineStage<Message> for ValidationStage {
    fn name(&self) -> &'static str {
        "validation"
    }

    fn priority(&self) -> u8 {
        10 // High priority, executes first
    }

    async fn process(
        &mut self,
        input: Message,
        context: &mut dyn ProcessContext,
    ) -> PipelineResult<Message> {
        // Basic message validation
        if let Err(e) = MessageValidator::validate_message(&input) {
            log::warn!("Message validation failed: {:?}", e);
            return PipelineResult::Error(e);
        }

        // Timestamp validation
        if !self.skip_timestamp_validation {
            let now = context.time_sync().now_utc();
            let msg_time = input.header.timestamp;
            let diff = (now.unix_timestamp() - msg_time.unix_timestamp()).abs();

            if diff > 3600 {
                // Messages older than 1 hour are considered invalid
                log::warn!("Message timestamp too old: {} seconds", diff);
                return PipelineResult::Error(Error::InvalidCommand);
            }
        }

        log::debug!("Message validation passed for ID: {}", input.header.id);
        PipelineResult::Continue(input)
    }
}

pub struct AuthenticationStage {
    allowed_sources: Option<alloc::vec::Vec<NodeId>>,
    require_target_match: bool,
}

impl AuthenticationStage {
    pub fn new() -> Self {
        Self {
            allowed_sources: None,
            require_target_match: true,
        }
    }

    pub fn allow_sources(mut self, sources: alloc::vec::Vec<NodeId>) -> Self {
        self.allowed_sources = Some(sources);
        self
    }

    pub fn skip_target_check(mut self) -> Self {
        self.require_target_match = false;
        self
    }
}

#[async_trait::async_trait]
impl PipelineStage<Message> for AuthenticationStage {
    fn name(&self) -> &'static str {
        "authentication"
    }

    fn priority(&self) -> u8 {
        20
    }

    async fn process(
        &mut self,
        input: Message,
        _context: &mut dyn ProcessContext,
    ) -> PipelineResult<Message> {
        // Check if source is allowed
        if let Some(ref allowed) = self.allowed_sources {
            if !allowed.contains(&input.header.source) {
                log::warn!("Unauthorized source: {:?}", input.header.source);
                return PipelineResult::Error(Error::InvalidCommand);
            }
        }

        // Check target match (device verification)
        if self.require_target_match {
            // TODO: Specific target validation logic can be added here
            // e.g., check if device MAC address matches
        }

        log::debug!(
            "Authentication passed for message from {:?}",
            input.header.source
        );
        PipelineResult::Continue(input)
    }
}

pub struct RateLimitStage {
    max_messages_per_second: u32,
    last_message_time: Option<OffsetDateTime>,
    message_count: u32,
    window_start: Option<OffsetDateTime>,
}

impl RateLimitStage {
    pub fn new(max_per_second: u32) -> Self {
        Self {
            max_messages_per_second: max_per_second,
            last_message_time: None,
            message_count: 0,
            window_start: None,
        }
    }
}

#[async_trait::async_trait]
impl PipelineStage<Message> for RateLimitStage {
    fn name(&self) -> &'static str {
        "rate_limit"
    }

    fn priority(&self) -> u8 {
        30
    }

    async fn process(
        &mut self,
        input: Message,
        context: &mut dyn ProcessContext,
    ) -> PipelineResult<Message> {
        let now = context.time_sync().now_utc();

        // Initialize time window
        if self.window_start.is_none() {
            self.window_start = Some(now);
            self.message_count = 0;
        }

        // Check if counter needs reset
        if let Some(window_start) = self.window_start {
            let elapsed = (now.unix_timestamp() - window_start.unix_timestamp()) as u32;
            if elapsed >= 1 {
                // Reset time window
                self.window_start = Some(now);
                self.message_count = 0;
            }
        }

        self.message_count += 1;

        // Check if limit exceeded
        if self.message_count > self.max_messages_per_second {
            log::warn!("Rate limit exceeded: {} messages/sec", self.message_count);
            return PipelineResult::Error(Error::InvalidCommand);
        }

        self.last_message_time = Some(now);
        PipelineResult::Continue(input)
    }
}

pub struct PriorityStage;

#[async_trait::async_trait]
impl PipelineStage<Message> for PriorityStage {
    fn name(&self) -> &'static str {
        "priority"
    }

    fn priority(&self) -> u8 {
        40
    }

    async fn process(
        &mut self,
        input: Message,
        _context: &mut dyn ProcessContext,
    ) -> PipelineResult<Message> {
        // Emergency messages get priority processing
        if input.header.priority == Priority::Emergency {
            log::info!("Processing emergency message: {}", input.header.id);
        }

        PipelineResult::Continue(input)
    }
}

pub struct RoutingStage {
    supported_payloads: alloc::vec::Vec<&'static str>,
}

impl RoutingStage {
    pub fn new() -> Self {
        Self {
            supported_payloads: alloc::vec::Vec::new(),
        }
    }

    pub fn support_payload(mut self, payload_type: &'static str) -> Self {
        self.supported_payloads.push(payload_type);
        self
    }

    pub fn support_edge_commands(self) -> Self {
        self.support_payload("EdgeCommand")
    }

    pub fn support_device_reports(self) -> Self {
        self.support_payload("DeviceReport")
    }

    pub fn support_cloud_commands(self) -> Self {
        self.support_payload("CloudCommand")
    }
}

#[async_trait::async_trait]
impl PipelineStage<Message> for RoutingStage {
    fn name(&self) -> &'static str {
        "routing"
    }

    fn priority(&self) -> u8 {
        50
    }

    async fn process(
        &mut self,
        input: Message,
        _context: &mut dyn ProcessContext,
    ) -> PipelineResult<Message> {
        if self.supported_payloads.is_empty() {
            // If no supported payload types specified, allow all
            return PipelineResult::Continue(input);
        }

        let payload_type = match &input.payload {
            MessagePayload::CloudCommand(_) => "CloudCommand",
            MessagePayload::EdgeReport(_) => "EdgeReport",
            MessagePayload::EdgeCommand(_) => "EdgeCommand",
            MessagePayload::DeviceReport(_) => "DeviceReport",
            MessagePayload::Acknowledge(_) => "Acknowledge",
            MessagePayload::Error(_) => "Error",
        };

        if self.supported_payloads.contains(&payload_type) {
            log::debug!("Routing {} message", payload_type);
            PipelineResult::Continue(input)
        } else {
            log::debug!("Unsupported payload type: {}", payload_type);
            PipelineResult::Skip
        }
    }
}

/// Error handling stage
pub struct ErrorHandlingStage {
    log_errors: bool,
    auto_recover: bool,
}

impl ErrorHandlingStage {
    pub fn new() -> Self {
        Self {
            log_errors: true,
            auto_recover: false,
        }
    }

    pub fn with_logging(mut self, enabled: bool) -> Self {
        self.log_errors = enabled;
        self
    }

    pub fn with_auto_recovery(mut self, enabled: bool) -> Self {
        self.auto_recover = enabled;
        self
    }
}

#[async_trait::async_trait]
impl PipelineStage<Message> for ErrorHandlingStage {
    fn name(&self) -> &'static str {
        "error_handling"
    }

    fn priority(&self) -> u8 {
        200 // Low priority, executes last
    }

    fn should_execute(&self, _input: &Message, context: &dyn ProcessContext) -> bool {
        // Only execute when there's an error
        context.get_error().is_some()
    }

    async fn process(
        &mut self,
        input: Message,
        context: &mut dyn ProcessContext,
    ) -> PipelineResult<Message> {
        if let Some(error) = context.get_error() {
            if self.log_errors {
                log::error!("Pipeline error occurred: {:?}", error);
            }

            if self.auto_recover {
                // Attempt automatic recovery
                match error {
                    Error::NetworkError => {
                        log::info!("Attempting network error recovery");
                        // Recovery logic can be added here
                    }
                    Error::TimeoutError => {
                        log::info!("Attempting timeout error recovery");
                        // Recovery logic can be added here
                    }
                    _ => {}
                }
            }

            // Generate error response message
            if let Some(source) = context.current_message().map(|m| &m.header.source) {
                let error_response = context.message_builder().create_error_response(
                    source.clone(),
                    lumisync_api::ErrorPayload {
                        original_msg_id: Some(input.header.id),
                        code: lumisync_api::ErrorCode::InternalError,
                        message: alloc::format!("{:?}", error),
                    },
                    context.time_sync().now_utc(),
                );

                return PipelineResult::Complete(Some(error_response));
            }
        }

        PipelineResult::Continue(input)
    }
}

pub struct LoggingStage {
    log_level: LogLevel,
    include_payload: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
}

impl LoggingStage {
    pub fn new(level: LogLevel) -> Self {
        Self {
            log_level: level,
            include_payload: false,
        }
    }

    pub fn include_payload(mut self) -> Self {
        self.include_payload = true;
        self
    }
}

#[async_trait::async_trait]
impl PipelineStage<Message> for LoggingStage {
    fn name(&self) -> &'static str {
        "logging"
    }

    fn priority(&self) -> u8 {
        5 // Very high priority for early logging
    }

    async fn process(
        &mut self,
        input: Message,
        _context: &mut dyn ProcessContext,
    ) -> PipelineResult<Message> {
        let message_info = if self.include_payload {
            alloc::format!(
                "Message {} from {:?} to {:?}: {:?}",
                input.header.id,
                input.header.source,
                input.header.target,
                input.payload
            )
        } else {
            alloc::format!(
                "Message {} from {:?} to {:?}",
                input.header.id,
                input.header.source,
                input.header.target
            )
        };

        match self.log_level {
            LogLevel::Debug => log::debug!("{}", message_info),
            LogLevel::Info => log::info!("{}", message_info),
            LogLevel::Warn => log::warn!("{}", message_info),
        }

        PipelineResult::Continue(input)
    }
}

#[cfg(test)]
mod tests {
    use lumisync_api::{MessageHeader, NodeId, Priority};
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::handler::context::MockContext;

    use super::*;

    fn create_test_message() -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Device([1, 2, 3, 4, 5, 6]),
            },
            payload: MessagePayload::EdgeCommand(lumisync_api::EdgeCommand::Actuator {
                actuator_id: 1,
                sequence: 1,
                command: lumisync_api::ActuatorCommand::RequestStatus,
            }),
        }
    }

    #[tokio::test]
    async fn test_rate_limit_stage() {
        let mut stage = RateLimitStage::new(2); // Max 2 messages per second
        let mut context = MockContext::new();

        // First message should pass
        let message1 = create_test_message();
        let result1 = stage.process(message1, &mut context).await;
        assert!(result1.is_continue());

        // Second message should pass
        let message2 = create_test_message();
        let result2 = stage.process(message2, &mut context).await;
        assert!(result2.is_continue());

        // Third message should be rate limited
        let message3 = create_test_message();
        let result3 = stage.process(message3, &mut context).await;
        assert!(result3.is_error());
    }

    #[tokio::test]
    async fn test_routing_stage() {
        let mut stage = RoutingStage::new().support_edge_commands();
        let mut context = MockContext::new();
        let message = create_test_message();

        let result = stage.process(message, &mut context).await;
        assert!(result.is_continue());
    }
}
