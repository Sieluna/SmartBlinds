use alloc::vec::Vec;

use lumisync_api::protocols::{Protocol, SerializationProtocol};
use lumisync_api::{Message, MessageHeader, MessagePayload, NodeId, Priority};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Error, Result};

pub struct MessageBuilder {
    protocol: SerializationProtocol,
}

impl MessageBuilder {
    pub fn new() -> Self {
        Self {
            protocol: SerializationProtocol::default(),
        }
    }

    pub fn with_protocol(protocol: SerializationProtocol) -> Self {
        Self { protocol }
    }

    /// Create device status message
    pub fn device_status(
        &self,
        source: NodeId,
        target: NodeId,
        actuator_id: lumisync_api::Id,
        window_position: u8,
        battery_level: u8,
        error_code: u8,
    ) -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source,
                target,
            },
            payload: MessagePayload::DeviceReport(lumisync_api::DeviceReport::Status {
                actuator_id,
                window_data: lumisync_api::WindowData {
                    target_position: window_position,
                },
                battery_level,
                error_code,
            }),
        }
    }

    /// Create device control message
    pub fn actuator_command(
        &self,
        source: NodeId,
        target: NodeId,
        actuator_id: lumisync_api::Id,
        sequence: u16,
        position: u8,
    ) -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source,
                target,
            },
            payload: MessagePayload::EdgeCommand(lumisync_api::EdgeCommand::Actuator {
                actuator_id,
                sequence,
                command: lumisync_api::ActuatorCommand::SetWindowPosition(position),
            }),
        }
    }

    pub fn serialize(&self, message: &Message) -> Result<Vec<u8>> {
        self.protocol
            .serialize(message)
            .map_err(|_| Error::SerializationError)
    }

    pub fn deserialize(&self, data: &[u8]) -> Result<Message> {
        self.protocol
            .deserialize(data)
            .map_err(|_| Error::SerializationError)
    }
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}
