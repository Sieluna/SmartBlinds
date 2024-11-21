use alloc::sync::Arc;
use alloc::vec::Vec;

use lumisync_api::message::*;
use lumisync_api::transport::{deserialize, serialize};
use lumisync_api::{Id, Message, Protocol, SensorData, WindowData};
use time::OffsetDateTime;

use crate::{Error, Result};

use super::uuid_generator::UuidGenerator;

pub struct MessageBuilder {
    protocol: Protocol,
    node_id: NodeId,
    uuid_generator: Arc<dyn UuidGenerator>,
}

impl MessageBuilder {
    pub fn new(protocol: Protocol, uuid_generator: Arc<dyn UuidGenerator>) -> Self {
        Self {
            protocol,
            node_id: NodeId::Cloud,
            uuid_generator,
        }
    }

    pub fn with_node_id(mut self, node_id: NodeId) -> Self {
        self.node_id = node_id;
        self
    }

    pub fn with_uuid_generator(mut self, generator: Arc<dyn UuidGenerator>) -> Self {
        self.uuid_generator = generator;
        self
    }

    pub fn set_node_id(&mut self, node_id: NodeId) {
        self.node_id = node_id;
    }

    pub fn set_uuid_generator(&mut self, generator: Arc<dyn UuidGenerator>) {
        self.uuid_generator = generator;
    }

    pub fn serialize(&self, message: &Message) -> Result<Vec<u8>> {
        serialize(self.protocol, message).map_err(|_| Error::SerializationError)
    }

    pub fn deserialize(&self, data: &[u8]) -> Result<Message> {
        deserialize(self.protocol, data).map_err(|_| Error::SerializationError)
    }

    /// Create a basic message header
    fn create_header(
        &self,
        target: NodeId,
        priority: Priority,
        timestamp: OffsetDateTime,
    ) -> MessageHeader {
        MessageHeader {
            id: self.uuid_generator.generate(),
            timestamp,
            priority,
            source: self.node_id.clone(),
            target,
        }
    }

    /// Create device status update message
    pub fn create_device_status(
        &self,
        target: NodeId,
        actuator_id: Id,
        window_data: WindowData,
        battery_level: u8,
        error_code: u8,
        relative_timestamp: u64,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Regular, timestamp),
            payload: MessagePayload::DeviceReport(DeviceReport::Status {
                actuator_id,
                window_data,
                battery_level,
                error_code,
                relative_timestamp,
            }),
        }
    }

    /// Create sensor data message
    pub fn create_sensor_data(
        &self,
        target: NodeId,
        actuator_id: Id,
        sensor_data: SensorData,
        relative_timestamp: u64,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Regular, timestamp),
            payload: MessagePayload::DeviceReport(DeviceReport::SensorData {
                actuator_id,
                sensor_data,
                relative_timestamp,
            }),
        }
    }

    /// Create health status message
    pub fn create_health_status(
        &self,
        target: NodeId,
        device_id: Id,
        cpu_usage: f32,
        memory_usage: f32,
        battery_level: u8,
        signal_strength: i8,
        relative_timestamp: u64,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Regular, timestamp),
            payload: MessagePayload::DeviceReport(DeviceReport::HealthStatus {
                device_id,
                cpu_usage,
                memory_usage,
                battery_level,
                signal_strength,
                relative_timestamp,
            }),
        }
    }

    /// Create actuator command message
    pub fn create_actuator_command(
        &self,
        target: NodeId,
        actuator_id: Id,
        sequence: u16,
        command: ActuatorCommand,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Regular, timestamp),
            payload: MessagePayload::EdgeCommand(EdgeCommand::Actuator {
                actuator_id,
                sequence,
                command,
            }),
        }
    }

    /// Create health status request message
    pub fn create_health_status_request(
        &self,
        target: NodeId,
        actuator_id: Id,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Regular, timestamp),
            payload: MessagePayload::EdgeCommand(EdgeCommand::RequestHealthStatus { actuator_id }),
        }
    }

    /// Create sensor data request message
    pub fn create_sensor_data_request(
        &self,
        target: NodeId,
        actuator_id: Id,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Regular, timestamp),
            payload: MessagePayload::EdgeCommand(EdgeCommand::RequestSensorData { actuator_id }),
        }
    }

    /// Create time sync request message
    pub fn create_time_sync_request(
        &self,
        target: NodeId,
        local_time: OffsetDateTime,
        current_offset_ms: i64,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Regular, timestamp),
            payload: MessagePayload::EdgeReport(EdgeReport::RequestTimeSync {
                local_time,
                current_offset_ms,
            }),
        }
    }

    /// Create emergency message with high priority
    pub fn create_emergency_stop(
        &self,
        target: NodeId,
        actuator_id: Id,
        sequence: u16,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Emergency, timestamp),
            payload: MessagePayload::EdgeCommand(EdgeCommand::Actuator {
                actuator_id,
                sequence,
                command: ActuatorCommand::EmergencyStop,
            }),
        }
    }

    /// Create acknowledgment message
    pub fn create_acknowledgment(
        &self,
        target: NodeId,
        ack_payload: AckPayload,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Regular, timestamp),
            payload: MessagePayload::Acknowledge(ack_payload),
        }
    }

    /// Create error response message
    pub fn create_error_response(
        &self,
        target: NodeId,
        error_payload: ErrorPayload,
        timestamp: OffsetDateTime,
    ) -> Message {
        Message {
            header: self.create_header(target, Priority::Regular, timestamp),
            payload: MessagePayload::Error(error_payload),
        }
    }
}
