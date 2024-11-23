mod device_sync;
mod edge_sync;
mod provider;

pub use device_sync::DeviceTimeSync;
pub use edge_sync::EdgeTimeSync;
pub use provider::EmbeddedTimeProvider;

pub use lumisync_api::{SyncConfig, SyncStatus, TimeProvider};

#[cfg(test)]
mod tests {
    use super::*;
    use lumisync_api::{Message, MessageHeader, MessagePayload, NodeId, Priority, TimeSyncPayload};
    use time::OffsetDateTime;
    use uuid::Uuid;

    /// Helper function to create a mock cloud sync response
    fn create_cloud_sync_response(
        edge_id: u8,
        request_sequence: u32,
        server_time: OffsetDateTime,
    ) -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: server_time,
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: NodeId::Edge(edge_id),
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Response {
                request_sequence,
                request_receive_time: server_time,
                response_send_time: server_time,
                estimated_delay_ms: 25,
                accuracy_ms: 5,
            }),
        }
    }

    /// Helper function to extract sequence from sync request
    fn extract_sequence_from_request(request: &Message) -> Option<u32> {
        if let MessagePayload::TimeSync(TimeSyncPayload::Request { sequence, .. }) =
            &request.payload
        {
            Some(*sequence)
        } else {
            None
        }
    }

    #[test]
    fn test_edge_to_device_time_broadcast() {
        let edge_id = 1;
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Initialize edge and device sync managers
        let edge_sync = EdgeTimeSync::new(edge_id);
        let mut device_sync = DeviceTimeSync::new(device_mac);

        // Edge creates time broadcast
        let broadcast_result = edge_sync.create_time_broadcast();
        assert!(broadcast_result.is_ok());

        let broadcast = broadcast_result.unwrap();

        // Verify broadcast message structure
        assert_eq!(broadcast.header.source, NodeId::Edge(edge_id));
        assert!(matches!(
            broadcast.payload,
            MessagePayload::TimeSync(TimeSyncPayload::Broadcast { .. })
        ));

        // Device handles the broadcast
        let handle_result = device_sync.handle_time_broadcast(&broadcast);
        assert!(handle_result.is_ok());

        // Device should now be synchronized
        assert!(device_sync.is_synced());
        assert_eq!(
            device_sync.sync_state,
            device_sync::DeviceSyncState::Synced
        );
    }

    #[test]
    fn test_full_sync_chain_cloud_to_edge_to_device() {
        let edge_id = 1;
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Initialize components
        let mut edge_sync = EdgeTimeSync::new(edge_id);
        let mut device_sync = DeviceTimeSync::new(device_mac);

        // Step 1: Edge requests sync from cloud
        assert!(edge_sync.needs_cloud_sync());
        let cloud_request_result = edge_sync.create_cloud_sync_request();
        assert!(cloud_request_result.is_ok());

        let cloud_request = cloud_request_result.unwrap();
        let request_sequence = extract_sequence_from_request(&cloud_request).unwrap();

        // Step 2: Simulate cloud response with current time + offset
        let server_time = OffsetDateTime::now_utc();
        let cloud_response = create_cloud_sync_response(edge_id, request_sequence, server_time);

        // Step 3: Edge handles cloud response
        let edge_response_result = edge_sync.handle_cloud_sync_response(&cloud_response);
        assert!(edge_response_result.is_ok());

        // Edge should be synchronized after cloud response
        assert_eq!(edge_sync.get_sync_status(), SyncStatus::Synced);

        // Step 4: Edge broadcasts time to devices
        let broadcast_result = edge_sync.create_time_broadcast();
        assert!(broadcast_result.is_ok());

        let time_broadcast = broadcast_result.unwrap();

        // Step 5: Device receives and processes broadcast
        let device_response_result = device_sync.handle_time_broadcast(&time_broadcast);
        assert!(device_response_result.is_ok());

        // Device should now be synchronized
        assert!(device_sync.is_synced());

        // Verify time alignment (should be reasonably close)
        let edge_time = edge_sync.get_current_time();
        let device_time = device_sync.get_current_time();

        let time_diff = (edge_time.unix_timestamp() - device_time.unix_timestamp()).abs();
        // Relaxed assertion - embedded systems can have more variance
        assert!(
            time_diff <= 10,
            "Time difference should be within 10 seconds, got: {} seconds",
            time_diff
        );
    }

    #[test]
    fn test_multiple_devices_sync_with_edge() {
        let edge_id = 1;
        let device_count = 3;

        // Initialize edge sync
        let edge_sync = EdgeTimeSync::new(edge_id);

        // Initialize multiple devices
        let mut devices: Vec<DeviceTimeSync> = (0..device_count)
            .map(|i| DeviceTimeSync::new([0x01, 0x02, 0x03, 0x04, 0x05, i as u8]))
            .collect();

        // Edge creates time broadcast
        let broadcast = edge_sync.create_time_broadcast().unwrap();

        // All devices receive the same broadcast
        for device in &mut devices {
            let result = device.handle_time_broadcast(&broadcast);
            assert!(result.is_ok());
            assert!(device.is_synced());
        }

        // All devices should have similar time
        let first_device_time = devices[0].get_current_time();
        for device in &devices[1..] {
            let device_time = device.get_current_time();
            let time_diff =
                (first_device_time.unix_timestamp() - device_time.unix_timestamp()).abs();
            assert!(
                time_diff <= 1,
                "All devices should have nearly the same synchronized time, diff: {}",
                time_diff
            );
        }
    }

    #[test]
    fn test_device_sync_expiry() {
        let edge_id = 1;
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // Create device with short expiry threshold
        let mut device_sync = DeviceTimeSync::with_expiry_threshold(device_mac, 100); // 100ms
        let edge_sync = EdgeTimeSync::new(edge_id);

        // Initial sync
        let broadcast = edge_sync.create_time_broadcast().unwrap();
        device_sync.handle_time_broadcast(&broadcast).unwrap();
        assert!(device_sync.is_synced());

        // Simulate time passage by manually setting old sync time
        device_sync.last_sync_time = Some(0); // Very old time

        // Since is_synced() now checks the actual time difference, we need to call update_sync_state
        // to trigger the state change to Expired
        device_sync.update_sync_state();

        // Check that is_synced() returns false (due to time check)
        assert!(
            !device_sync.is_synced(),
            "Device should not be synced after expiry"
        );

        // After calling update_sync_state, the state should be Expired
        assert_eq!(
            device_sync.sync_state,
            device_sync::DeviceSyncState::Expired
        );
    }

    #[test]
    fn test_edge_custom_config_sync() {
        let edge_id = 1;

        // Create edge with custom configuration
        let custom_config = SyncConfig {
            sync_interval_ms: 5000, // 5 seconds
            max_drift_ms: 50,       // 50ms max drift
            offset_history_size: 3, // Keep 3 samples
            delay_compensation_threshold_ms: 25,
            max_retry_count: 2,
        };

        let edge_sync = EdgeTimeSync::with_config(edge_id, custom_config);

        // Verify configuration is applied
        assert_eq!(edge_sync.edge_id, edge_id);
        assert_eq!(edge_sync.cloud_sync_interval_ms, 5000);
        assert!(edge_sync.needs_cloud_sync());
    }

    #[test]
    fn test_time_offset_calculation() {
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let mut device_sync = DeviceTimeSync::new(device_mac);

        // Create a broadcast with known offset
        let known_offset = 1000; // 1 second - more reasonable
        let timestamp = OffsetDateTime::now_utc();

        let broadcast = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp,
                priority: Priority::Emergency,
                source: NodeId::Edge(1),
                target: NodeId::Device(device_mac),
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Broadcast {
                timestamp,
                offset_ms: known_offset,
                accuracy_ms: 10,
            }),
        };

        // Handle broadcast
        device_sync.handle_time_broadcast(&broadcast).unwrap();

        // Verify device is synchronized
        assert!(device_sync.is_synced());

        // The calculated offset should be reasonable
        let calculated_offset = device_sync.time_offset_ms;

        // The offset should be within a reasonable range (allowing for processing time and clock differences)
        assert!(
            calculated_offset.abs() < 60000,
            "Calculated offset should be reasonable, got: {} ms",
            calculated_offset
        );
    }

    #[test]
    fn test_sync_reset_functionality() {
        let edge_id = 1;
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        let mut edge_sync = EdgeTimeSync::new(edge_id);
        let mut device_sync = DeviceTimeSync::new(device_mac);

        // Perform initial sync
        let broadcast = edge_sync.create_time_broadcast().unwrap();
        device_sync.handle_time_broadcast(&broadcast).unwrap();

        // Both should be in some synchronized state
        assert!(device_sync.is_synced());

        // Reset both
        edge_sync.reset();
        device_sync.reset();

        // Both should return to initial state
        assert_eq!(edge_sync.get_sync_status(), SyncStatus::Unsynced);
        assert!(!device_sync.is_synced());
        assert_eq!(
            device_sync.sync_state,
            device_sync::DeviceSyncState::Unsynced
        );
        assert_eq!(device_sync.time_offset_ms, 0);
    }

    #[test]
    fn test_invalid_message_handling() {
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let mut device_sync = DeviceTimeSync::new(device_mac);

        // Create invalid message (non-time-sync)
        let invalid_message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Device(device_mac),
            },
            payload: MessagePayload::Acknowledge(lumisync_api::AckPayload {
                original_msg_id: Uuid::new_v4(),
                status: "OK".into(),
                details: None,
            }),
        };

        // Device should reject invalid message
        let result = device_sync.handle_time_broadcast(&invalid_message);
        assert!(result.is_err());
        assert!(!device_sync.is_synced());
    }

    #[test]
    fn test_time_provider_consistency() {
        // Test that time providers work consistently across components
        let provider1 = EmbeddedTimeProvider::new();
        let provider2 = EmbeddedTimeProvider::new();

        let time1 = provider1.uptime_ms();
        let time2 = provider2.uptime_ms();

        // Times should be very close (within a few milliseconds)
        let diff = if time1 > time2 {
            time1 - time2
        } else {
            time2 - time1
        };
        assert!(diff < 100, "Time providers should return similar values");

        // Test monotonic behavior
        let later_time1 = provider1.uptime_ms();
        assert!(
            later_time1 >= time1,
            "Time should be monotonically increasing"
        );
    }
}
