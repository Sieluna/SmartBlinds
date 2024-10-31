use alloc::collections::BTreeMap;

use lumisync_api::{CloudCommand, MessagePayload, NodeId};

use crate::communication::MessageTransport;
use crate::protocol::message::MessageBuilder;
use crate::{Error, Result};

pub struct EdgeCommunicator<T, B>
where
    T: MessageTransport,
    B: MessageTransport,
{
    /// TCP transport layer (for communicating with cloud server)
    tcp_transport: T,
    /// BLE transport layer (for communicating with devices)
    ble_transport: B,
    /// Message builder
    message_builder: MessageBuilder,
    /// Current Edge node ID
    node_id: NodeId,
}

impl<T, B> EdgeCommunicator<T, B>
where
    T: MessageTransport,
    B: MessageTransport,
{
    pub fn new(tcp_transport: T, ble_transport: B, edge_id: u8) -> Self {
        Self {
            tcp_transport,
            ble_transport,
            message_builder: MessageBuilder::new(),
            node_id: NodeId::Edge(edge_id),
        }
    }

    /// Handle messages from cloud server
    pub async fn handle_cloud_message(&mut self) -> Result<()> {
        if let Ok(Some(message)) = self.tcp_transport.receive_message().await {
            match &message.payload {
                MessagePayload::CloudCommand(cloud_cmd) => {
                    self.process_cloud_command(cloud_cmd).await?;
                }
                _ => {
                    // Ignore other message types
                }
            }
        }
        Ok(())
    }

    /// Process cloud commands
    async fn process_cloud_command(&mut self, command: &CloudCommand) -> Result<()> {
        match command {
            CloudCommand::ControlDevices {
                region_id: _,
                commands,
            } => {
                self.distribute_device_commands(commands).await?;
            }
            CloudCommand::ConfigureWindow { window_id, plan: _ } => {
                // Simplified implementation: convert window configuration to position command
                let position = 50; // Default position, should be parsed from plan in actual implementation
                self.send_window_command(*window_id, position).await?;
            }
            _ => {
                // Other commands not handled yet
            }
        }
        Ok(())
    }

    /// Distribute device commands
    async fn distribute_device_commands(
        &mut self,
        commands: &BTreeMap<lumisync_api::Id, lumisync_api::Command>,
    ) -> Result<()> {
        for (device_id, command) in commands {
            match command {
                lumisync_api::Command::SetWindow { device_id: _, data } => {
                    self.send_window_command(*device_id, data.target_position)
                        .await?;
                }
                _ => {
                    // Other command types not handled yet
                }
            }
        }
        Ok(())
    }

    /// Send window control command
    async fn send_window_command(
        &mut self,
        device_id: lumisync_api::Id,
        position: u8,
    ) -> Result<()> {
        // Create device MAC address (simplified implementation)
        let device_mac = self.device_id_to_mac(device_id);
        let target_node = NodeId::Device(device_mac);

        let message = self.message_builder.actuator_command(
            self.node_id.clone(),
            target_node,
            device_id,
            1, // Sequence number
            position,
        );

        self.ble_transport
            .send_message(&message)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(())
    }

    /// Simplified device ID to MAC address conversion
    fn device_id_to_mac(&self, device_id: lumisync_api::Id) -> [u8; 6] {
        let device_id = device_id.unsigned_abs(); // Ensure positive number
        [
            0x12,
            0x34,
            0x56,
            (device_id >> 16) as u8,
            (device_id >> 8) as u8,
            device_id as u8,
        ]
    }

    /// Send device status to cloud server
    pub async fn report_device_status(
        &mut self,
        device_id: lumisync_api::Id,
        position: u8,
        battery: u8,
    ) -> Result<()> {
        let message = self.message_builder.device_status(
            self.node_id.clone(),
            NodeId::Cloud,
            device_id,
            position,
            battery,
            0, // No error
        );

        self.tcp_transport
            .send_message(&message)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use lumisync_api::Message;

    use super::*;

    struct MockTransport {
        messages: Vec<Message>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                messages: Vec::new(),
            }
        }
    }

    impl MessageTransport for MockTransport {
        type Error = Error;

        async fn send_message(&mut self, message: &Message) -> Result<()> {
            self.messages.push(message.clone());
            Ok(())
        }

        async fn receive_message(&mut self) -> Result<Option<Message>> {
            Ok(self.messages.pop())
        }
    }

    #[test]
    fn test_device_id_to_mac() {
        let tcp_transport = MockTransport::new();
        let ble_transport = MockTransport::new();
        let edge = EdgeCommunicator::new(tcp_transport, ble_transport, 1);

        let mac = edge.device_id_to_mac(0x123456);
        assert_eq!(mac, [0x12, 0x34, 0x56, 0x12, 0x34, 0x56]);
    }
}
