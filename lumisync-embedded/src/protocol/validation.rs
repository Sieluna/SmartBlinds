use lumisync_api::{ActuatorCommand, DeviceReport, EdgeCommand, Message, MessagePayload, NodeId};

use crate::{Error, Result};

pub struct MessageValidator;

impl MessageValidator {
    /// Validate message integrity
    pub fn validate_message(message: &Message) -> Result<()> {
        // Check message header
        if message.header.id.is_nil() {
            return Err(Error::InvalidCommand);
        }

        // Check node ID validity
        if let NodeId::Device(mac) = &message.header.source {
            if mac.iter().all(|&b| b == 0) {
                return Err(Error::InvalidCommand);
            }
        }

        // Check payload validity
        match &message.payload {
            MessagePayload::EdgeCommand(cmd) => {
                Self::validate_edge_command(cmd)?;
            }
            MessagePayload::DeviceReport(report) => {
                Self::validate_device_report(report)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn validate_edge_command(command: &EdgeCommand) -> Result<()> {
        if let EdgeCommand::Actuator {
            actuator_id,
            sequence: _,
            command,
        } = command
        {
            if *actuator_id == 0 {
                return Err(Error::InvalidCommand);
            }

            if let ActuatorCommand::SetWindowPosition(pos) = command {
                if *pos > 100 {
                    return Err(Error::InvalidCommand);
                }
            }
        }
        Ok(())
    }

    fn validate_device_report(report: &DeviceReport) -> Result<()> {
        match report {
            DeviceReport::Status {
                actuator_id,
                window_data,
                battery_level,
                ..
            } => {
                if *actuator_id == 0 {
                    return Err(Error::InvalidState);
                }
                if window_data.target_position > 100 {
                    return Err(Error::InvalidState);
                }
                if *battery_level > 100 {
                    return Err(Error::InvalidState);
                }
            }
            DeviceReport::SensorData { actuator_id, .. } => {
                if *actuator_id == 0 {
                    return Err(Error::InvalidState);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lumisync_api::*;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn test_message_validator() {
        let valid_message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::EdgeCommand(EdgeCommand::Actuator {
                actuator_id: 42,
                sequence: 1,
                command: ActuatorCommand::SetWindowPosition(75),
            }),
        };

        assert!(MessageValidator::validate_message(&valid_message).is_ok());

        // Test invalid position
        let invalid_message = Message {
            header: valid_message.header.clone(),
            payload: MessagePayload::EdgeCommand(EdgeCommand::Actuator {
                actuator_id: 42,
                sequence: 1,
                command: ActuatorCommand::SetWindowPosition(150), // Invalid position
            }),
        };

        assert!(MessageValidator::validate_message(&invalid_message).is_err());
    }
}
