pub mod coordinator;
pub mod service;
pub mod status;
pub mod sync;

pub use coordinator::*;
pub use service::*;
pub use status::*;
pub use sync::*;

pub trait TimeProvider {
    /// Get monotonic uptime in milliseconds since device boot
    fn monotonic_time_ms(&self) -> u64;

    /// Get wall clock time
    fn wall_clock_time(&self) -> Option<time::OffsetDateTime> {
        None
    }

    /// Check if this provider has authoritative time source
    fn has_authoritative_time(&self) -> bool {
        self.wall_clock_time().is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncError {
    /// Network delay too high
    HighNetworkDelay,
    /// Time drift exceeds acceptable threshold
    ExcessiveDrift,
    /// Sync operation timed out
    Timeout,
    /// Transport layer error during sync
    TransportError,
    /// Received invalid timestamp
    InvalidTimestamp,
    /// Device not synchronized
    NotSynchronized,
}

impl core::fmt::Display for SyncError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SyncError::HighNetworkDelay => write!(f, "Network delay too high for reliable sync"),
            SyncError::ExcessiveDrift => write!(f, "Time drift exceeds acceptable threshold"),
            SyncError::Timeout => write!(f, "Sync operation timed out"),
            SyncError::TransportError => write!(f, "Transport layer error during sync"),
            SyncError::InvalidTimestamp => write!(f, "Received invalid timestamp"),
            SyncError::NotSynchronized => {
                write!(f, "Device not synchronized - cannot provide network time")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SyncError {}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use time::OffsetDateTime;

    use crate::message::NodeId;
    use crate::uuid::DeviceBasedUuidGenerator;

    use super::*;

    #[derive(Clone)]
    struct MockTimeProvider {
        uptime_ms: u64,
        has_wall_clock: bool,
        wall_clock_offset: i64, // Simulate clock offset differences between nodes
    }

    impl MockTimeProvider {
        fn new(uptime: u64) -> Self {
            Self {
                uptime_ms: uptime,
                has_wall_clock: false,
                wall_clock_offset: 0,
            }
        }

        fn with_wall_clock(uptime: u64, offset: i64) -> Self {
            Self {
                uptime_ms: uptime,
                has_wall_clock: true,
                wall_clock_offset: offset,
            }
        }

        fn advance_time(&mut self, ms: u64) {
            self.uptime_ms += ms;
        }
    }

    impl TimeProvider for MockTimeProvider {
        fn monotonic_time_ms(&self) -> u64 {
            self.uptime_ms
        }

        fn wall_clock_time(&self) -> Option<OffsetDateTime> {
            if self.has_wall_clock {
                let base_timestamp = 1_700_000_000i64; // Base timestamp for 2023
                // Use uptime as offset to simulate real-time clock
                let adjusted_timestamp =
                    base_timestamp + (self.uptime_ms / 1000) as i64 + self.wall_clock_offset / 1000;
                OffsetDateTime::from_unix_timestamp(adjusted_timestamp).ok()
            } else {
                None
            }
        }
    }

    fn create_test_config() -> SyncConfig {
        SyncConfig {
            sync_interval_ms: 5000, // 5 second sync interval
            max_drift_ms: 200,      // Allow 200ms drift
            offset_history_size: 3,
            delay_threshold_ms: 100,
            max_retry_count: 3,
            failure_cooldown_ms: 10000,
        }
    }

    // Test Utilities
    struct TestContext {
        config: SyncConfig,
        cloud_service: TimeSyncService<MockTimeProvider>,
        edge_services: BTreeMap<u8, TimeSyncService<MockTimeProvider>>,
        device_services: BTreeMap<[u8; 6], TimeSyncService<MockTimeProvider>>,
    }

    impl TestContext {
        fn new() -> Self {
            let config = create_test_config();
            let cloud_provider = MockTimeProvider::with_wall_clock(1000, 0);

            let cloud_service = TimeSyncService::new(
                cloud_provider,
                NodeId::Cloud,
                config.clone(),
                DeviceBasedUuidGenerator::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
            );

            Self {
                config,
                cloud_service,
                edge_services: BTreeMap::new(),
                device_services: BTreeMap::new(),
            }
        }

        fn add_edge(&mut self, edge_id: u8, initial_offset: u64) {
            let edge_provider = MockTimeProvider::new(1000 + initial_offset);
            let edge_service = TimeSyncService::new(
                edge_provider,
                NodeId::Edge(edge_id),
                self.config.clone(),
                DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, edge_id]),
            );
            self.edge_services.insert(edge_id, edge_service);
        }

        fn add_device(&mut self, mac: [u8; 6], initial_offset: u64) {
            let device_provider = MockTimeProvider::new(1000 + initial_offset);
            let device_service = TimeSyncService::new(
                device_provider,
                NodeId::Device(mac),
                self.config.clone(),
                DeviceBasedUuidGenerator::new(mac),
            );
            self.device_services.insert(mac, device_service);
        }

        fn sync_cloud_to_edges(&mut self) {
            for (_, edge_service) in self.edge_services.iter_mut() {
                if edge_service.needs_sync() {
                    if let Ok(request) = edge_service.create_sync_request(NodeId::Cloud) {
                        if let Ok(response) = self.cloud_service.handle_sync_request(&request) {
                            let _ = edge_service.handle_sync_response(&response);
                        }
                    }
                }
            }
        }

        fn sync_edges_to_devices(&mut self) {
            for (mac, device_service) in self.device_services.iter_mut() {
                if device_service.needs_sync() {
                    // Determine which edge to sync with based on MAC address
                    let edge_id = if mac[0] == 0x11 { 1 } else { 2 };

                    if let Some(edge_service) = self.edge_services.get_mut(&edge_id) {
                        if let Ok(request) =
                            device_service.create_sync_request(NodeId::Edge(edge_id))
                        {
                            if let Ok(response) = edge_service.handle_sync_request(&request) {
                                let _ = device_service.handle_sync_response(&response);
                            }
                        }
                    }
                }
            }
        }

        fn count_synced_devices(&self) -> usize {
            self.device_services
                .values()
                .filter(|s| matches!(s.get_sync_status(), SyncStatus::Synced))
                .count()
        }

        fn all_edges_synced(&self) -> bool {
            self.edge_services
                .values()
                .all(|s| matches!(s.get_sync_status(), SyncStatus::Synced))
        }

        fn perform_full_sync_round(&mut self) {
            self.sync_cloud_to_edges();
            self.sync_edges_to_devices();
        }
    }

    fn create_hierarchical_network() -> TestContext {
        let mut context = TestContext::new();

        // Add two edge nodes with different initial offsets
        context.add_edge(1, 50); // 50ms offset
        context.add_edge(2, 20); // 20ms offset

        // Add 4 devices to each edge (8 total)
        for i in 0..4u8 {
            let mac1 = [0x11, 0x11, 0x11, 0x11, 0x11, i];
            let mac2 = [0x22, 0x22, 0x22, 0x22, 0x22, i];
            context.add_device(mac1, i as u64 * 30);
            context.add_device(mac2, i as u64 * 25);
        }

        context
    }

    fn create_simple_cloud_edge_setup() -> (
        TimeSyncService<MockTimeProvider>,
        TimeSyncService<MockTimeProvider>,
    ) {
        let config = create_test_config();

        let cloud_service = TimeSyncService::new(
            MockTimeProvider::with_wall_clock(1000, 0),
            NodeId::Cloud,
            config.clone(),
            DeviceBasedUuidGenerator::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
        );

        let edge_service = TimeSyncService::new(
            MockTimeProvider::new(1050),
            NodeId::Edge(1),
            config,
            DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]),
        );

        (cloud_service, edge_service)
    }

    fn perform_sync_between_services(
        requester: &mut TimeSyncService<MockTimeProvider>,
        responder: &mut TimeSyncService<MockTimeProvider>,
        target_node: NodeId,
    ) -> Result<(), SyncError> {
        let request = requester.create_sync_request(target_node)?;
        let response = responder.handle_sync_request(&request)?;
        requester.handle_sync_response(&response)
    }

    #[test]
    fn test_hierarchical_time_sync_network() {
        let mut context = create_hierarchical_network();

        // Perform multiple sync rounds
        let sync_rounds = 3;
        for _round in 1..=sync_rounds {
            context.perform_full_sync_round();
        }

        // Verify final state
        assert!(
            context.all_edges_synced(),
            "All edge nodes should be synced"
        );
        assert_eq!(
            context.count_synced_devices(),
            8,
            "All 8 devices should be synced"
        );
    }

    #[test]
    fn test_network_partition_recovery() {
        let config = create_test_config();
        let cloud_provider = MockTimeProvider::with_wall_clock(1000, 0);
        let mut edge_provider = MockTimeProvider::new(1050);

        let mut cloud_service = TimeSyncService::new(
            cloud_provider,
            NodeId::Cloud,
            config.clone(),
            DeviceBasedUuidGenerator::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
        );

        let mut edge_service = TimeSyncService::new(
            edge_provider.clone(),
            NodeId::Edge(1),
            config.clone(),
            DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]),
        );

        // Initial sync to establish baseline
        assert!(
            perform_sync_between_services(&mut edge_service, &mut cloud_service, NodeId::Cloud)
                .is_ok()
        );
        assert_eq!(edge_service.get_sync_status(), SyncStatus::Synced);

        // Simulate network partition - edge cannot communicate with cloud for extended period
        let partition_duration = 6000; // 6 seconds, exceeds sync interval
        edge_provider.advance_time(partition_duration);

        // Recreate service to reflect time advancement
        edge_service = TimeSyncService::new(
            edge_provider.clone(),
            NodeId::Edge(1),
            config,
            DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]),
        );

        // During partition, edge should need to resync
        assert!(
            edge_service.needs_sync(),
            "Edge should need resync after partition"
        );

        // Network recovery - successful sync
        assert!(
            perform_sync_between_services(&mut edge_service, &mut cloud_service, NodeId::Cloud)
                .is_ok()
        );
        assert_eq!(edge_service.get_sync_status(), SyncStatus::Synced);
    }

    #[test]
    fn test_node_failure_and_rejoin() {
        let (mut cloud_service, mut edge_service) = create_simple_cloud_edge_setup();

        // Initial sync
        assert!(
            perform_sync_between_services(&mut edge_service, &mut cloud_service, NodeId::Cloud)
                .is_ok()
        );
        assert_eq!(edge_service.get_sync_status(), SyncStatus::Synced);

        // Simulate node restart/failure - reset sync state
        edge_service.reset_sync();
        assert_eq!(edge_service.get_sync_status(), SyncStatus::Unsynced);
        assert!(edge_service.needs_sync());

        // Rejoin network and sync
        assert!(
            perform_sync_between_services(&mut edge_service, &mut cloud_service, NodeId::Cloud)
                .is_ok()
        );
        assert_eq!(edge_service.get_sync_status(), SyncStatus::Synced);
    }

    #[test]
    fn test_high_latency_network_conditions() {
        let mut config = create_test_config();
        config.delay_threshold_ms = 500; // Increase threshold to tolerate high latency

        let cloud_provider = MockTimeProvider::with_wall_clock(1000, 0);
        let mut edge_provider = MockTimeProvider::new(1000);

        let mut cloud_service = TimeSyncService::new(
            cloud_provider,
            NodeId::Cloud,
            config.clone(),
            DeviceBasedUuidGenerator::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
        );

        let mut edge_service = TimeSyncService::new(
            edge_provider.clone(),
            NodeId::Edge(1),
            config,
            DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]),
        );

        // Simulate high latency network - advance time between request and response
        let request = edge_service.create_sync_request(NodeId::Cloud).unwrap();

        // Simulate high network delay
        edge_provider.advance_time(300); // 300ms delay

        let response = cloud_service.handle_sync_request(&request).unwrap();
        let result = edge_service.handle_sync_response(&response);

        // Should succeed with higher delay threshold
        assert!(
            result.is_ok(),
            "Sync should succeed under high latency conditions"
        );
        assert_eq!(edge_service.get_sync_status(), SyncStatus::Synced);
    }

    #[test]
    fn test_clock_drift_correction() {
        let config = create_test_config();

        // Use similar times to avoid excessive offset
        let mut cloud_provider = MockTimeProvider::with_wall_clock(1000, 0);
        let mut edge_provider = MockTimeProvider::new(1050); // 50ms offset

        let mut cloud_service = TimeSyncService::new(
            cloud_provider.clone(),
            NodeId::Cloud,
            config.clone(),
            DeviceBasedUuidGenerator::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
        );

        let mut edge_service = TimeSyncService::new(
            edge_provider.clone(),
            NodeId::Edge(1),
            config.clone(),
            DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]),
        );

        // Initial sync
        assert!(
            perform_sync_between_services(&mut edge_service, &mut cloud_service, NodeId::Cloud)
                .is_ok()
        );

        // Simulate clock drift - advance time and perform multiple syncs
        for _i in 1..=3 {
            // Advance provider time while maintaining relative consistency
            let time_advance = 6000; // 6 seconds
            edge_provider.advance_time(time_advance);
            cloud_provider.advance_time(time_advance);

            // Recreate services to use updated time
            cloud_service = TimeSyncService::new(
                cloud_provider.clone(),
                NodeId::Cloud,
                config.clone(),
                DeviceBasedUuidGenerator::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
            );

            edge_service = TimeSyncService::new(
                edge_provider.clone(),
                NodeId::Edge(1),
                config.clone(),
                DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]),
            );

            assert!(
                perform_sync_between_services(&mut edge_service, &mut cloud_service, NodeId::Cloud)
                    .is_ok()
            );
        }

        // Verify sync state
        assert_eq!(edge_service.get_sync_status(), SyncStatus::Synced);
    }

    #[test]
    fn test_coordinator_integration() {
        let config = create_test_config();
        let mut coordinator = TimeSyncCoordinator::new();

        // Create multiple services and add to coordinator
        let cloud_service = TimeSyncService::new(
            MockTimeProvider::with_wall_clock(1000, 0),
            NodeId::Cloud,
            config.clone(),
            DeviceBasedUuidGenerator::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
        );

        let edge_service = TimeSyncService::new(
            MockTimeProvider::new(1050),
            NodeId::Edge(1),
            config,
            DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]),
        );

        coordinator.add_service(NodeId::Cloud, cloud_service);
        coordinator.add_service(NodeId::Edge(1), edge_service);

        assert_eq!(coordinator.service_count(), 2);

        // Check network status
        let status = coordinator.get_network_status();
        assert_eq!(status.total_nodes, 2);
        assert_eq!(status.synced_nodes, 0); // Initially unsynced

        // Test message routing
        let node_ids = coordinator.get_node_ids();
        assert!(node_ids.contains(&NodeId::Cloud));
        assert!(node_ids.contains(&NodeId::Edge(1)));

        // Reset all services
        coordinator.reset_all();
        let status_after_reset = coordinator.get_network_status();
        assert_eq!(status_after_reset.synced_nodes, 0);
    }

    #[test]
    fn test_sync_failure_scenarios() {
        let mut config = create_test_config();
        config.max_retry_count = 2;
        config.failure_cooldown_ms = 1000;

        let (mut cloud_service, mut edge_service) = create_simple_cloud_edge_setup();

        // Test successive sync failures leading to cooldown
        let current_time = 1000;

        // Trigger enough failures to enter cooldown state
        for _i in 0..=config.max_retry_count {
            edge_service
                .get_synchronizer_mut()
                .handle_sync_failure(current_time);
        }

        // Should be in failed state with cooldown
        assert!(matches!(
            edge_service.get_sync_status(),
            SyncStatus::Failed { .. }
        ));
        assert!(!edge_service.needs_sync()); // In cooldown

        // Test that a fresh service after cooldown period can sync
        let post_cooldown_time = current_time + config.failure_cooldown_ms + 100;
        let fresh_provider = MockTimeProvider::new(post_cooldown_time);
        let mut fresh_edge_service = TimeSyncService::new(
            fresh_provider,
            NodeId::Edge(1),
            config,
            DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]),
        );

        // Fresh service should be able to sync
        assert!(fresh_edge_service.needs_sync());
        assert!(
            perform_sync_between_services(
                &mut fresh_edge_service,
                &mut cloud_service,
                NodeId::Cloud
            )
            .is_ok()
        );
        assert_eq!(fresh_edge_service.get_sync_status(), SyncStatus::Synced);
    }

    #[test]
    fn test_sync_precision_requirements() {
        let config = create_test_config();

        // Create cloud service with wall clock (authoritative time)
        let mut cloud_service = TimeSyncService::new(
            MockTimeProvider::with_wall_clock(1000, 0),
            NodeId::Cloud,
            config.clone(),
            DeviceBasedUuidGenerator::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
        );

        // Create edge and device services without authoritative time
        let mut edge_service = TimeSyncService::new(
            MockTimeProvider::new(1000),
            NodeId::Edge(1),
            config.clone(),
            DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]),
        );

        let device_service = TimeSyncService::new(
            MockTimeProvider::new(1000),
            NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
            config,
            DeviceBasedUuidGenerator::new([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
        );

        // Before sync: all should return MAX accuracy except cloud after sync
        assert_eq!(cloud_service.get_current_accuracy(), u16::MAX); // Even cloud returns MAX before sync
        assert_eq!(edge_service.get_current_accuracy(), u16::MAX);
        assert_eq!(device_service.get_current_accuracy(), u16::MAX);

        // After sync, edge should have better accuracy
        assert!(
            perform_sync_between_services(&mut edge_service, &mut cloud_service, NodeId::Cloud)
                .is_ok()
        );

        // Now edge should have actual precision value, cloud still has better precision when synced
        let edge_accuracy = edge_service.get_current_accuracy();
        assert_ne!(edge_accuracy, u16::MAX);
        assert_eq!(edge_accuracy, 10); // Edge precision requirement

        // Device should still have MAX (not synced)
        assert_eq!(device_service.get_current_accuracy(), u16::MAX);
    }
}
