mod device_sync;
mod edge_sync;
mod provider;

pub use device_sync::{DeviceSyncState, DeviceSyncStats, DeviceTimeSync};
pub use edge_sync::{EdgeSyncStats, EdgeTimeSync};
pub use provider::EmbeddedTimeProvider;

#[cfg(test)]
mod tests {
    use lumisync_api::message::*;
    use lumisync_api::time::*;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::*;

    /// Create mock cloud sync response
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

    /// Extract sequence from sync request
    fn extract_sequence_from_request(request: &Message) -> Option<u32> {
        if let MessagePayload::TimeSync(TimeSyncPayload::Request { sequence, .. }) =
            &request.payload
        {
            Some(*sequence)
        } else {
            None
        }
    }

    /// Sync edge with cloud first
    fn sync_edge_with_cloud(edge_sync: &mut EdgeTimeSync) {
        let cloud_request = edge_sync.create_cloud_sync_request().unwrap();
        let request_sequence = extract_sequence_from_request(&cloud_request).unwrap();
        let server_time = OffsetDateTime::now_utc();
        let cloud_response =
            create_cloud_sync_response(edge_sync.edge_id, request_sequence, server_time);
        edge_sync
            .handle_cloud_sync_response(&cloud_response)
            .unwrap();
    }

    #[test]
    fn test_edge_to_device_time_broadcast() {
        let edge_id = 1;
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        let mut edge_sync = EdgeTimeSync::new(edge_id);
        let mut device_sync = DeviceTimeSync::new(device_mac);

        sync_edge_with_cloud(&mut edge_sync);
        assert_eq!(edge_sync.get_sync_status(), SyncStatus::Synced);

        let broadcast_result = edge_sync.create_time_broadcast();
        assert!(broadcast_result.is_ok());

        let broadcast = broadcast_result.unwrap();

        assert_eq!(broadcast.header.source, NodeId::Edge(edge_id));
        assert!(matches!(
            broadcast.payload,
            MessagePayload::TimeSync(TimeSyncPayload::Broadcast { .. })
        ));

        let handle_result = device_sync.handle_time_broadcast(&broadcast);
        assert!(handle_result.is_ok());

        assert!(device_sync.is_synced());
        assert_eq!(device_sync.sync_state, device_sync::DeviceSyncState::Synced);
    }

    #[test]
    fn test_full_sync_chain_cloud_to_edge_to_device() {
        let edge_id = 1;
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        let mut edge_sync = EdgeTimeSync::new(edge_id);
        let mut device_sync = DeviceTimeSync::new(device_mac);

        // Step 1: Edge requests sync from cloud
        assert!(edge_sync.needs_cloud_sync());
        let cloud_request_result = edge_sync.create_cloud_sync_request();
        assert!(cloud_request_result.is_ok());

        let cloud_request = cloud_request_result.unwrap();
        let request_sequence = extract_sequence_from_request(&cloud_request).unwrap();

        // Step 2: Simulate cloud response
        let server_time = OffsetDateTime::now_utc();
        let cloud_response = create_cloud_sync_response(edge_id, request_sequence, server_time);

        // Step 3: Edge handles cloud response
        let edge_response_result = edge_sync.handle_cloud_sync_response(&cloud_response);
        assert!(edge_response_result.is_ok());

        assert_eq!(edge_sync.get_sync_status(), SyncStatus::Synced);

        // Step 4: Edge broadcasts time to devices
        let broadcast_result = edge_sync.create_time_broadcast();
        assert!(broadcast_result.is_ok());

        let time_broadcast = broadcast_result.unwrap();

        // Step 5: Device receives and processes broadcast
        let device_response_result = device_sync.handle_time_broadcast(&time_broadcast);
        assert!(device_response_result.is_ok());

        assert!(device_sync.is_synced());

        // Verify time alignment
        let edge_time = edge_sync.get_current_time();
        let device_time = device_sync.get_current_time();

        let time_diff = (edge_time.unix_timestamp() - device_time.unix_timestamp()).abs();
        assert!(
            time_diff <= 60,
            "Time difference should be within 60 seconds for test, got: {} seconds. Edge: {}, Device: {}",
            time_diff,
            edge_time.unix_timestamp(),
            device_time.unix_timestamp()
        );
    }

    #[test]
    fn test_multiple_devices_sync_with_edge() {
        let edge_id = 1;
        let device_count = 3;

        let mut edge_sync = EdgeTimeSync::new(edge_id);
        sync_edge_with_cloud(&mut edge_sync);

        let mut devices: Vec<DeviceTimeSync> = (0..device_count)
            .map(|i| DeviceTimeSync::new([0x01, 0x02, 0x03, 0x04, 0x05, i as u8]))
            .collect();

        let broadcast = edge_sync.create_time_broadcast().unwrap();

        for device in &mut devices {
            let result = device.handle_time_broadcast(&broadcast);
            assert!(result.is_ok());
            assert!(device.is_synced());
        }

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

        let mut device_sync = DeviceTimeSync::with_expiry_threshold(device_mac, 100);
        let mut edge_sync = EdgeTimeSync::new(edge_id);

        sync_edge_with_cloud(&mut edge_sync);

        let broadcast = edge_sync.create_time_broadcast().unwrap();
        device_sync.handle_time_broadcast(&broadcast).unwrap();
        assert!(device_sync.is_synced());

        // Simulate time passage by setting old sync time
        device_sync.last_sync_time = Some(0);

        std::thread::sleep(std::time::Duration::from_millis(110));

        device_sync.update_sync_state();

        assert!(
            !device_sync.is_synced(),
            "Device should not be synced after expiry"
        );

        assert_eq!(
            device_sync.sync_state,
            device_sync::DeviceSyncState::Expired
        );
    }

    #[test]
    fn test_time_offset_calculation() {
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let mut device_sync = DeviceTimeSync::new(device_mac);

        let known_offset = 1000;
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

        device_sync.handle_time_broadcast(&broadcast).unwrap();

        assert!(device_sync.is_synced());

        let device_time = device_sync.get_current_time();
        let expected_time = timestamp.unix_timestamp() + (known_offset / 1000) as i64;
        let time_diff = (device_time.unix_timestamp() - expected_time).abs();
        assert!(
            time_diff <= 2,
            "Device time should be accurate, got difference: {} seconds. Device: {}, Expected: {}",
            time_diff,
            device_time.unix_timestamp(),
            expected_time
        );
    }

    #[test]
    fn test_invalid_message_handling() {
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let mut device_sync = DeviceTimeSync::new(device_mac);

        let invalid_message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Device(device_mac),
            },
            payload: MessagePayload::Acknowledge(AckPayload {
                original_msg_id: Uuid::new_v4(),
                status: "OK".into(),
                details: None,
            }),
        };

        let result = device_sync.handle_time_broadcast(&invalid_message);
        assert!(result.is_err());
        assert!(!device_sync.is_synced());
    }

    #[test]
    fn test_device_sync_statistics() {
        let device_mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let mut device_sync = DeviceTimeSync::new(device_mac);
        let mut edge_sync = EdgeTimeSync::new(1);

        sync_edge_with_cloud(&mut edge_sync);

        // Test initial stats
        assert_eq!(device_sync.stats.sync_count, 0);
        assert_eq!(device_sync.stats.success_count, 0);
        assert_eq!(device_sync.get_sync_quality(), 0.0);

        // Successful sync
        let broadcast = edge_sync.create_time_broadcast().unwrap();
        device_sync.handle_time_broadcast(&broadcast).unwrap();

        assert_eq!(device_sync.stats.sync_count, 1);
        assert_eq!(device_sync.stats.success_count, 1);
        assert_eq!(device_sync.get_sync_quality(), 1.0);

        // Failed sync (wrong source)
        let invalid_broadcast = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Device([0xFF; 6]), // Wrong source
                target: NodeId::Device(device_mac),
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Broadcast {
                timestamp: OffsetDateTime::now_utc(),
                offset_ms: 0,
                accuracy_ms: 10,
            }),
        };

        let _ = device_sync.handle_time_broadcast(&invalid_broadcast);
        assert_eq!(device_sync.stats.sync_count, 2);
        assert_eq!(device_sync.stats.success_count, 1); // Still 1 success
        assert_eq!(device_sync.get_sync_quality(), 0.5);
    }

    #[test]
    fn test_edge_sync_statistics() {
        let mut edge_sync = EdgeTimeSync::new(1);

        // Test initial stats
        assert_eq!(edge_sync.stats.cloud_syncs, 0);
        assert_eq!(edge_sync.get_cloud_sync_quality(), 0.0);
        assert!(edge_sync.should_sync_with_cloud());

        // Simulate cloud sync
        let _request = edge_sync.create_cloud_sync_request().unwrap();
        assert_eq!(edge_sync.stats.cloud_syncs, 1);

        // Simulate successful response
        let response = create_cloud_sync_response(1, 1, OffsetDateTime::now_utc());
        edge_sync.handle_cloud_sync_response(&response).unwrap();

        assert_eq!(edge_sync.stats.cloud_successes, 1);
        assert_eq!(edge_sync.get_cloud_sync_quality(), 1.0);

        // Test broadcast stats
        let _broadcast = edge_sync.create_time_broadcast().unwrap();
        assert_eq!(edge_sync.stats.broadcasts_sent, 1);
    }

    #[test]
    fn test_network_failure_recovery() {
        let edge_id = 1;
        let mut edge_sync = EdgeTimeSync::new(edge_id);
        let mut devices: Vec<DeviceTimeSync> = (0..3)
            .map(|i| DeviceTimeSync::new([0x11, 0x22, 0x33, 0x44, 0x55, i as u8]))
            .collect();

        // Initial sync
        sync_edge_with_cloud(&mut edge_sync);
        let broadcast = edge_sync.create_time_broadcast().unwrap();
        for device in devices.iter_mut() {
            device.handle_time_broadcast(&broadcast).unwrap();
        }

        // All devices should be synced
        assert_eq!(devices.iter().filter(|d| d.is_synced()).count(), 3);

        // Simulate device failures
        devices[0].reset(); // Device 1 fails
        devices[2].reset(); // Device 3 fails

        // Only device 2 should be synced
        assert_eq!(devices.iter().filter(|d| d.is_synced()).count(), 1);

        // Recovery broadcast
        let recovery_broadcast = edge_sync.create_time_broadcast().unwrap();
        for device in devices.iter_mut() {
            let _ = device.handle_time_broadcast(&recovery_broadcast);
        }

        // All devices should recover
        assert_eq!(devices.iter().filter(|d| d.is_synced()).count(), 3);
    }
}
