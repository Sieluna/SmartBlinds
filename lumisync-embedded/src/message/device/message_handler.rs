use embedded_io_async::{ErrorType, Read, Write};
use lumisync_api::message::*;
use lumisync_api::transport::{AsyncMessageTransport, TransportError};

use crate::protocol::message::MessageBuilder;
use crate::time::TimeSync;
use crate::{Error, Result};

use super::motor_control::MotorController;
use super::safety_manager::SafetyManager;
use super::status_manager::StatusManager;
use crate::stepper::Motor;

pub struct MessageHandler {
    message_builder: MessageBuilder,
    device_mac: [u8; 6],
}

impl MessageHandler {
    pub fn new(message_builder: MessageBuilder, device_mac: [u8; 6]) -> Self {
        Self {
            message_builder,
            device_mac,
        }
    }

    /// Handle incoming messages from Edge device
    pub async fn handle_edge_message<IO, M>(
        &mut self,
        transport: &mut AsyncMessageTransport<IO>,
        motor_controller: &mut MotorController<M>,
        status_manager: &mut StatusManager,
        safety_manager: &mut SafetyManager,
        time_sync: &TimeSync,
    ) -> Result<()>
    where
        IO: Read + Write + ErrorType + Unpin,
        IO::Error: core::fmt::Debug,
        M: Motor,
    {
        match transport.receive_message::<lumisync_api::Message>().await {
            Ok((message, _, _)) => {
                if let Err(e) = self.validate_edge_message(&message, time_sync) {
                    log::warn!("Invalid edge message: {:?}", e);
                    return Ok(());
                }

                match &message.payload {
                    MessagePayload::EdgeCommand(edge_cmd) => {
                        self.process_edge_command(
                            edge_cmd,
                            &message.header.source,
                            transport,
                            motor_controller,
                            status_manager,
                            safety_manager,
                            time_sync,
                        )
                        .await?;
                    }
                    _ => {
                        log::debug!("Ignoring non-command message from edge");
                    }
                }
            }
            Err(TransportError::Io(_)) => {
                // No message available, not an error
            }
            Err(e) => {
                log::error!("Edge message transport error: {:?}", e);
                return Err(Error::SerializationError);
            }
        }
        Ok(())
    }

    /// Validate incoming edge messages
    fn validate_edge_message(
        &self,
        message: &lumisync_api::Message,
        time_sync: &TimeSync,
    ) -> Result<()> {
        if !matches!(message.header.source, NodeId::Edge(_)) {
            return Err(Error::InvalidCommand);
        }

        if !matches!(message.header.target, NodeId::Device(mac) if mac == self.device_mac) {
            return Err(Error::InvalidCommand);
        }

        let now = time_sync.now_utc();
        let msg_time = message.header.timestamp;
        let diff = (now.unix_timestamp() - msg_time.unix_timestamp()).abs();

        if diff > 7200 {
            log::warn!("Edge message timestamp drift: {} seconds", diff);
        }

        Ok(())
    }

    /// Process Edge commands
    async fn process_edge_command<IO, M>(
        &mut self,
        command: &EdgeCommand,
        source: &NodeId,
        transport: &mut AsyncMessageTransport<IO>,
        motor_controller: &mut MotorController<M>,
        status_manager: &mut StatusManager,
        safety_manager: &mut SafetyManager,
        time_sync: &TimeSync,
    ) -> Result<()>
    where
        IO: Read + Write + ErrorType + Unpin,
        IO::Error: core::fmt::Debug,
        M: Motor,
    {
        match command {
            EdgeCommand::Actuator {
                actuator_id,
                sequence,
                command,
            } => {
                if *actuator_id == status_manager.device_status.device_id {
                    log::debug!("Processing actuator command (seq: {})", sequence);

                    self.execute_actuator_command(
                        command,
                        source,
                        *sequence,
                        transport,
                        motor_controller,
                        status_manager,
                        safety_manager,
                        time_sync,
                    )
                    .await?;
                }
            }
            EdgeCommand::RequestHealthStatus { actuator_id } => {
                if *actuator_id == status_manager.device_status.device_id {
                    self.send_health_status(source, transport, status_manager, time_sync)
                        .await?;
                }
            }
            EdgeCommand::RequestSensorData { actuator_id } => {
                if *actuator_id == status_manager.device_status.device_id {
                    self.send_sensor_data(source, transport, status_manager, time_sync)
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Execute actuator commands
    async fn execute_actuator_command<IO, M>(
        &mut self,
        command: &ActuatorCommand,
        source: &NodeId,
        sequence: u16,
        transport: &mut AsyncMessageTransport<IO>,
        motor_controller: &mut MotorController<M>,
        status_manager: &mut StatusManager,
        safety_manager: &mut SafetyManager,
        time_sync: &TimeSync,
    ) -> Result<()>
    where
        IO: Read + Write + ErrorType + Unpin,
        IO::Error: core::fmt::Debug,
        M: Motor,
    {
        let command_result = match command {
            ActuatorCommand::SetWindowPosition(position) => {
                motor_controller
                    .set_window_position_safe(*position, time_sync)
                    .await
            }
            ActuatorCommand::RequestStatus => {
                self.send_status_update(
                    source,
                    transport,
                    motor_controller,
                    status_manager,
                    time_sync,
                )
                .await
            }
            ActuatorCommand::EmergencyStop => motor_controller.emergency_stop().await,
            ActuatorCommand::Calibrate => motor_controller.calibrate_safe(time_sync).await,
        };

        match command_result {
            Ok(_) => {
                if !matches!(command, ActuatorCommand::RequestStatus) {
                    self.send_acknowledgment(source, sequence, "OK", transport, time_sync)
                        .await?;
                    self.send_status_update(
                        source,
                        transport,
                        motor_controller,
                        status_manager,
                        time_sync,
                    )
                    .await?;
                }
            }
            Err(e) => {
                log::error!("Command execution failed: {:?}", e);
                status_manager.increment_error_count();

                self.send_error_response(
                    source,
                    sequence,
                    &format!("{:?}", e),
                    transport,
                    time_sync,
                )
                .await?;
                self.send_status_update(
                    source,
                    transport,
                    motor_controller,
                    status_manager,
                    time_sync,
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Send status update
    async fn send_status_update<IO, M>(
        &mut self,
        target: &NodeId,
        transport: &mut AsyncMessageTransport<IO>,
        motor_controller: &MotorController<M>,
        status_manager: &StatusManager,
        time_sync: &TimeSync,
    ) -> Result<()>
    where
        IO: Read + Write + ErrorType + Unpin,
        IO::Error: core::fmt::Debug,
        M: Motor,
    {
        let relative_ts = time_sync.uptime_ms();
        let error_code =
            status_manager.get_error_code_from_motor_state(&motor_controller.motor_state);

        let message = self.message_builder.create_device_status(
            target.clone(),
            status_manager.device_status.device_id,
            lumisync_api::WindowData {
                target_position: motor_controller.current_position,
            },
            status_manager.device_status.battery_level,
            error_code,
            relative_ts,
            time_sync.now_utc(),
        );

        transport
            .send_message(&message, None, None)
            .await
            .map_err(|_| Error::NetworkError)?;

        log::debug!("Sent status update to {:?}", target);
        Ok(())
    }

    /// Send health status
    async fn send_health_status<IO>(
        &mut self,
        target: &NodeId,
        transport: &mut AsyncMessageTransport<IO>,
        status_manager: &StatusManager,
        time_sync: &TimeSync,
    ) -> Result<()>
    where
        IO: Read + Write + ErrorType + Unpin,
        IO::Error: core::fmt::Debug,
    {
        let relative_ts = time_sync.uptime_ms();
        let (cpu_usage, memory_usage) = status_manager.get_system_metrics();

        let message = self.message_builder.create_health_status(
            target.clone(),
            status_manager.device_status.device_id,
            cpu_usage,
            memory_usage,
            status_manager.device_status.battery_level,
            -50, // signal_strength
            relative_ts,
            time_sync.now_utc(),
        );

        transport
            .send_message(&message, None, None)
            .await
            .map_err(|_| Error::NetworkError)?;

        log::debug!("Sent health status to {:?}", target);
        Ok(())
    }

    /// Send sensor data
    async fn send_sensor_data<IO>(
        &mut self,
        target: &NodeId,
        transport: &mut AsyncMessageTransport<IO>,
        status_manager: &StatusManager,
        time_sync: &TimeSync,
    ) -> Result<()>
    where
        IO: Read + Write + ErrorType + Unpin,
        IO::Error: core::fmt::Debug,
    {
        let relative_ts = time_sync.uptime_ms();
        let sensor_data = status_manager.generate_sensor_data(relative_ts);

        let message = self.message_builder.create_sensor_data(
            target.clone(),
            status_manager.device_status.device_id,
            sensor_data,
            relative_ts,
            time_sync.now_utc(),
        );

        transport
            .send_message(&message, None, None)
            .await
            .map_err(|_| Error::NetworkError)?;

        log::debug!("Sent sensor data to {:?}", target);
        Ok(())
    }

    /// Send acknowledgment
    async fn send_acknowledgment<IO>(
        &mut self,
        target: &NodeId,
        sequence: u16,
        status: &str,
        transport: &mut AsyncMessageTransport<IO>,
        time_sync: &TimeSync,
    ) -> Result<()>
    where
        IO: Read + Write + ErrorType + Unpin,
        IO::Error: core::fmt::Debug,
    {
        let ack_payload = AckPayload {
            original_msg_id: uuid::Uuid::nil(),
            status: status.to_string(),
            details: Some(format!("Sequence: {}", sequence)),
        };

        let message = self.message_builder.create_acknowledgment(
            target.clone(),
            ack_payload,
            time_sync.now_utc(),
        );

        transport
            .send_message(&message, None, None)
            .await
            .map_err(|_| Error::NetworkError)?;

        log::debug!("Sent acknowledgment to {:?}: {}", target, status);
        Ok(())
    }

    /// Send error response
    async fn send_error_response<IO>(
        &mut self,
        target: &NodeId,
        sequence: u16,
        error_msg: &str,
        transport: &mut AsyncMessageTransport<IO>,
        time_sync: &TimeSync,
    ) -> Result<()>
    where
        IO: Read + Write + ErrorType + Unpin,
        IO::Error: core::fmt::Debug,
    {
        let error_payload = ErrorPayload {
            original_msg_id: None,
            code: ErrorCode::HardwareFailure,
            message: format!("Seq {}: {}", sequence, error_msg),
        };

        let message = self.message_builder.create_error_response(
            target.clone(),
            error_payload,
            time_sync.now_utc(),
        );

        transport
            .send_message(&message, None, None)
            .await
            .map_err(|_| Error::NetworkError)?;

        log::debug!("Sent error response to {:?}: {}", target, error_msg);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    use alloc::vec::Vec;

    use lumisync_api::Protocol;
    use lumisync_api::transport::AsyncMessageTransport;
    use uuid::Uuid;

    use crate::protocol::{DeviceBasedUuidGenerator, MessageBuilder};
    use crate::stepper::{Motor, Stepper};
    use crate::time::TimeSync;

    use super::*;

    #[derive(Clone)]
    struct MockMotor {
        enabled: bool,
    }

    impl MockMotor {
        fn new() -> Self {
            Self { enabled: false }
        }
    }

    impl Motor for MockMotor {
        fn step(&mut self, _step: i64) {}

        fn enable(&mut self) {
            self.enabled = true;
        }

        fn disable(&mut self) {
            self.enabled = false;
        }
    }

    struct MockIO {
        read_buffer: Vec<u8>,
        write_buffer: Vec<u8>,
        read_pos: usize,
    }

    impl MockIO {
        fn new() -> Self {
            Self {
                read_buffer: Vec::new(),
                write_buffer: Vec::new(),
                read_pos: 0,
            }
        }
    }

    impl embedded_io_async::ErrorType for MockIO {
        type Error = crate::Error;
    }

    impl embedded_io_async::Read for MockIO {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            if self.read_pos >= self.read_buffer.len() {
                return Ok(0); // EOF
            }

            let available = self.read_buffer.len() - self.read_pos;
            let to_read = buf.len().min(available);
            buf[..to_read]
                .copy_from_slice(&self.read_buffer[self.read_pos..self.read_pos + to_read]);
            self.read_pos += to_read;
            Ok(to_read)
        }
    }

    impl embedded_io_async::Write for MockIO {
        async fn write(&mut self, buf: &[u8]) -> Result<usize> {
            self.write_buffer.extend_from_slice(buf);
            Ok(buf.len())
        }

        async fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_message_validation() {
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let uuid_generator = Arc::new(DeviceBasedUuidGenerator::new(device_mac, 42));
        let message_builder = MessageBuilder::new(Protocol::default(), uuid_generator)
            .with_node_id(NodeId::Device(device_mac));

        let handler = MessageHandler::new(message_builder, device_mac);
        let time_sync = TimeSync::new();

        // Valid message
        let valid_msg = lumisync_api::Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: time_sync.now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Device(device_mac),
            },
            payload: MessagePayload::EdgeCommand(EdgeCommand::RequestHealthStatus {
                actuator_id: 42,
            }),
        };

        assert!(
            handler
                .validate_edge_message(&valid_msg, &time_sync)
                .is_ok()
        );

        // Invalid message (wrong target)
        let invalid_msg = lumisync_api::Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: time_sync.now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Device([0xFF; 6]), // Wrong MAC
            },
            payload: MessagePayload::EdgeCommand(EdgeCommand::RequestHealthStatus {
                actuator_id: 42,
            }),
        };

        assert!(
            handler
                .validate_edge_message(&invalid_msg, &time_sync)
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_actuator_command_processing() {
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let uuid_generator = Arc::new(DeviceBasedUuidGenerator::new(device_mac, 42));
        let message_builder = MessageBuilder::new(Protocol::default(), uuid_generator)
            .with_node_id(NodeId::Device(device_mac));

        let mut handler = MessageHandler::new(message_builder, device_mac);

        let mock_io = MockIO::new();
        let mut transport = AsyncMessageTransport::new(mock_io)
            .with_default_protocol(Protocol::Postcard)
            .with_crc(true);

        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let mut motor_controller = MotorController::new(stepper);
        let mut status_manager = StatusManager::new(42);
        let mut safety_manager = SafetyManager::new();
        let time_sync = TimeSync::new();

        // Test status request command
        let result = handler
            .execute_actuator_command(
                &ActuatorCommand::RequestStatus,
                &NodeId::Edge(1),
                1,
                &mut transport,
                &mut motor_controller,
                &mut status_manager,
                &mut safety_manager,
                &time_sync,
            )
            .await;

        assert!(result.is_ok());
    }
}
