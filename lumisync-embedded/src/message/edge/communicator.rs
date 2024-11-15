use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use lumisync_api::SerializationProtocol;
use lumisync_api::WindowData;
use lumisync_api::message::*;
use time::OffsetDateTime;

use crate::message::MessageTransport;
use crate::protocol::message::MessageBuilder;
use crate::protocol::uuid_generator::DeviceBasedUuidGenerator;
use crate::time::TimeSync;
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
    /// Time synchronization manager
    time_sync: TimeSync,
}

impl<T, B> EdgeCommunicator<T, B>
where
    T: MessageTransport,
    B: MessageTransport,
{
    pub fn new(tcp_transport: T, ble_transport: B, edge_id: u8) -> Self {
        let node_id = NodeId::Edge(edge_id);

        // Generate device MAC based on edge_id for UUID generation
        let device_mac = [0xED, 0xED, 0x00, 0xDE, 0xDE, edge_id];
        let uuid_generator = Arc::new(DeviceBasedUuidGenerator::new(device_mac, edge_id as u32));

        Self {
            tcp_transport,
            ble_transport,
            message_builder: MessageBuilder::new(SerializationProtocol::default(), uuid_generator)
                .with_node_id(node_id),
            time_sync: TimeSync::new(),
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
                let position = 50; // Default position
                self.send_window_command(*window_id, position).await?;
            }
            CloudCommand::TimeSync { cloud_time } => {
                self.time_sync.sync(*cloud_time);
                log::info!("Edge time synced: {}", cloud_time);
            }
            _ => {}
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

        let message = self.message_builder.create_actuator_command(
            target_node,
            device_id,
            1, // sequence
            ActuatorCommand::SetWindowPosition(position),
            OffsetDateTime::UNIX_EPOCH,
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
        let message = self.message_builder.create_device_status(
            NodeId::Cloud,
            device_id,
            WindowData {
                target_position: position,
            },
            battery,
            0, // error_code
            self.time_sync.uptime_ms(),
            OffsetDateTime::UNIX_EPOCH,
        );

        self.tcp_transport
            .send_message(&message)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(())
    }

    /// Handle messages from devices (BLE)
    pub async fn handle_device_message(&mut self) -> Result<()> {
        if let Ok(Some(message)) = self.ble_transport.receive_message().await {
            match &message.payload {
                MessagePayload::DeviceReport(device_report) => {
                    self.process_device_report(device_report).await?;
                }
                _ => {
                    // Ignore other message types
                }
            }
        }
        Ok(())
    }

    /// Process device reports with timestamp conversion
    async fn process_device_report(&mut self, report: &lumisync_api::DeviceReport) -> Result<()> {
        match report {
            DeviceReport::Status {
                actuator_id,
                window_data,
                battery_level,
                error_code,
                relative_timestamp,
            } => {
                let utc_timestamp = self.time_sync.uptime_to_utc(*relative_timestamp);

                let message = self.message_builder.create_device_status(
                    NodeId::Cloud,
                    *actuator_id,
                    window_data.clone(),
                    *battery_level,
                    *error_code,
                    *relative_timestamp,
                    utc_timestamp,
                );

                self.tcp_transport
                    .send_message(&message)
                    .await
                    .map_err(|_| Error::NetworkError)?;
            }
            DeviceReport::SensorData {
                actuator_id,
                sensor_data,
                relative_timestamp,
            } => {
                let utc_timestamp = self.time_sync.uptime_to_utc(*relative_timestamp);

                let message = self.message_builder.create_sensor_data(
                    NodeId::Cloud,
                    *actuator_id,
                    sensor_data.clone(),
                    *relative_timestamp,
                    utc_timestamp,
                );

                self.tcp_transport
                    .send_message(&message)
                    .await
                    .map_err(|_| Error::NetworkError)?;
            }
            DeviceReport::HealthStatus {
                device_id,
                cpu_usage,
                memory_usage,
                battery_level,
                signal_strength,
                relative_timestamp,
            } => {
                let utc_timestamp = self.time_sync.uptime_to_utc(*relative_timestamp);

                let message = self.message_builder.create_health_status(
                    NodeId::Cloud,
                    *device_id,
                    *cpu_usage,
                    *memory_usage,
                    *battery_level,
                    *signal_strength,
                    *relative_timestamp,
                    utc_timestamp,
                );

                self.tcp_transport
                    .send_message(&message)
                    .await
                    .map_err(|_| Error::NetworkError)?;
            }
        }
        Ok(())
    }

    /// Request time sync from cloud
    pub async fn request_time_sync(&mut self) -> Result<()> {
        let message = self.message_builder.create_time_sync_request(
            NodeId::Cloud,
            self.time_sync.now_utc(),
            0, // Simplified: no offset needed
            self.time_sync.now_utc(),
        );

        self.tcp_transport
            .send_message(&message)
            .await
            .map_err(|_| Error::NetworkError)?;
        Ok(())
    }

    /// Check and sync time if needed
    pub async fn check_time_sync(&mut self) -> Result<()> {
        if self.time_sync.needs_sync() {
            self.request_time_sync().await?;
        }
        Ok(())
    }

    /// Get current time
    pub fn get_current_time(&self) -> OffsetDateTime {
        self.time_sync.now_utc()
    }

    /// Get sync status
    pub fn is_time_synced(&self) -> bool {
        self.time_sync.is_synced()
    }

    /// Get time sync status for monitoring (simplified)
    pub fn get_time_sync_status(&self) -> (bool, u64) {
        (self.time_sync.is_synced(), self.time_sync.uptime_ms())
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use lumisync_api::{Message, WindowData};
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::*;

    struct MockTransport {
        messages: Vec<Message>,
        receive_queue: Vec<Message>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                messages: Vec::new(),
                receive_queue: Vec::new(),
            }
        }

        fn add_message_to_receive(&mut self, message: Message) {
            self.receive_queue.push(message);
        }

        fn get_sent_messages(&self) -> &Vec<Message> {
            &self.messages
        }

        fn clear_sent_messages(&mut self) {
            self.messages.clear();
        }
    }

    impl MessageTransport for MockTransport {
        type Error = Error;

        async fn send_message(&mut self, message: &Message) -> Result<()> {
            self.messages.push(message.clone());
            Ok(())
        }

        async fn receive_message(&mut self) -> Result<Option<Message>> {
            Ok(self.receive_queue.pop())
        }
    }

    fn create_device_status_report(
        device_id: lumisync_api::Id,
        position: u8,
        battery: u8,
        relative_timestamp: u64,
    ) -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::DeviceReport(DeviceReport::Status {
                actuator_id: device_id,
                window_data: WindowData {
                    target_position: position,
                },
                battery_level: battery,
                error_code: 0,
                relative_timestamp,
            }),
        }
    }

    #[tokio::test]
    async fn test_timestamp_conversion_with_sync() {
        let tcp_transport = MockTransport::new();
        let ble_transport = MockTransport::new();
        let mut edge = EdgeCommunicator::new(tcp_transport, ble_transport, 1);

        // Sync with a known time
        let sync_time = OffsetDateTime::from_unix_timestamp(1609459200).unwrap();
        edge.time_sync.sync(sync_time);
        edge.tcp_transport.clear_sent_messages();

        // Test device with various uptimes
        let test_cases = [
            (1, 50, 90, 100),      // Device just booted
            (2, 75, 80, 86400000), // Device with 24h uptime
            (3, 25, 70, 0),        // Device at exact boot time
        ];

        for (device_id, position, battery, uptime) in test_cases {
            let report = create_device_status_report(device_id, position, battery, uptime);
            edge.ble_transport.add_message_to_receive(report);
            edge.handle_device_message().await.unwrap();
        }

        let sent_messages = edge.tcp_transport.get_sent_messages();
        assert_eq!(sent_messages.len(), 3);

        // Verify all timestamps are reasonable and after sync time
        for message in sent_messages.iter() {
            let timestamp = message.header.timestamp;
            assert!(
                timestamp.unix_timestamp() >= sync_time.unix_timestamp(),
                "Converted timestamp {} should be after sync time {}",
                timestamp,
                sync_time
            );
        }
    }

    #[test]
    fn test_device_id_to_mac_edge_cases() {
        let tcp_transport = MockTransport::new();
        let ble_transport = MockTransport::new();
        let edge = EdgeCommunicator::new(tcp_transport, ble_transport, 1);

        let test_cases = [
            (0, [0x12, 0x34, 0x56, 0x00, 0x00, 0x00]),
            (0xFFFFFF, [0x12, 0x34, 0x56, 0xFF, 0xFF, 0xFF]),
            (0x1000000, [0x12, 0x34, 0x56, 0x00, 0x00, 0x00]),
            (-1, [0x12, 0x34, 0x56, 0x00, 0x00, 0x01]),
        ];

        for (device_id, expected_mac) in test_cases {
            let mac = edge.device_id_to_mac(device_id);
            assert_eq!(
                mac, expected_mac,
                "MAC conversion failed for device_id {}: expected {:?}, got {:?}",
                device_id, expected_mac, mac
            );
        }
    }

    #[tokio::test]
    async fn test_time_sync_edge_conditions() {
        let tcp_transport = MockTransport::new();
        let ble_transport = MockTransport::new();
        let mut edge = EdgeCommunicator::new(tcp_transport, ble_transport, 1);

        // Test sync with extreme time values
        let extreme_future = OffsetDateTime::from_unix_timestamp(4000000000).unwrap();
        edge.time_sync.sync(extreme_future);

        let current_time = edge.get_current_time();
        assert!(current_time.unix_timestamp() > 3000000000);

        // Test multiple rapid syncs
        for i in 0..5 {
            let sync_time = OffsetDateTime::from_unix_timestamp(1609459200 + i * 60).unwrap();
            edge.time_sync.sync(sync_time);
            assert!(edge.is_time_synced());
        }
    }
}
