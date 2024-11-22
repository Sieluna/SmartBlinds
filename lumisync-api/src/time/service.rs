use alloc::collections::BTreeMap;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::message::*;

use super::{SyncConfig, SyncError, SyncStatus, TimeProvider, TimeSynchronizer};

pub struct TimeSyncService<T: TimeProvider> {
    time_provider: T,
    synchronizer: TimeSynchronizer,
    sequence_counter: u32,
    pending_requests: BTreeMap<u32, u64>, // sequence -> request_timestamp
    node_id: NodeId,
}

impl<T: TimeProvider> TimeSyncService<T> {
    pub fn new(time_provider: T, node_id: NodeId, config: SyncConfig) -> Self {
        Self {
            time_provider,
            synchronizer: TimeSynchronizer::new(node_id, config),
            sequence_counter: 0,
            pending_requests: BTreeMap::new(),
            node_id,
        }
    }

    /// Create time synchronization request
    pub fn create_sync_request(&mut self, target: NodeId) -> Result<Message, SyncError> {
        let current_time = self.time_provider.uptime_ms();

        if !self.synchronizer.needs_sync(current_time) {
            return Err(SyncError::InvalidTimestamp);
        }

        self.sequence_counter = self.sequence_counter.wrapping_add(1);
        let sequence = self.sequence_counter;

        self.pending_requests.insert(sequence, current_time);

        Ok(Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: self.get_current_time(),
                priority: Priority::Regular,
                source: self.node_id,
                target,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                sequence,
                send_time: self.get_current_time(),
                precision_ms: self.get_precision_requirement(),
            }),
        })
    }

    /// Handle time synchronization request
    pub fn handle_sync_request(&mut self, request: &Message) -> Result<Message, SyncError> {
        if let MessagePayload::TimeSync(TimeSyncPayload::Request {
            sequence,
            send_time,
            ..
        }) = &request.payload
        {
            let current_time = self.get_current_time();

            Ok(Message {
                header: MessageHeader {
                    id: Uuid::nil(),
                    timestamp: current_time,
                    priority: Priority::Regular,
                    source: self.node_id,
                    target: request.header.source,
                },
                payload: MessagePayload::TimeSync(TimeSyncPayload::Response {
                    request_sequence: *sequence,
                    request_receive_time: *send_time,
                    response_send_time: current_time,
                    estimated_delay_ms: self.estimate_network_delay(request.header.source),
                    accuracy_ms: self.get_current_accuracy(),
                }),
            })
        } else {
            Err(SyncError::InvalidTimestamp)
        }
    }

    /// Handle time synchronization response
    pub fn handle_sync_response(&mut self, response: &Message) -> Result<(), SyncError> {
        if let MessagePayload::TimeSync(TimeSyncPayload::Response {
            request_sequence,
            response_send_time,
            ..
        }) = &response.payload
        {
            if let Some(request_time) = self.pending_requests.remove(request_sequence) {
                let current_time = self.time_provider.uptime_ms();
                let response_time_ms = response_send_time.unix_timestamp() as u64 * 1000
                    + response_send_time.millisecond() as u64;

                self.synchronizer.handle_sync_response(
                    request_time,
                    response_time_ms,
                    current_time,
                )?;

                Ok(())
            } else {
                Err(SyncError::InvalidTimestamp)
            }
        } else {
            Err(SyncError::InvalidTimestamp)
        }
    }

    /// Create time broadcast message
    pub fn create_time_broadcast(&self) -> Result<Message, SyncError> {
        // Only edge nodes can broadcast time
        if !matches!(self.node_id, NodeId::Edge(_)) {
            return Err(SyncError::InvalidTimestamp);
        }

        Ok(Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: self.get_current_time(),
                priority: Priority::Regular,
                source: self.node_id,
                target: NodeId::Edge(255), // Broadcast address
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Broadcast {
                timestamp: self.get_current_time(),
                offset_ms: self.synchronizer.get_current_offset_ms(),
                accuracy_ms: self.get_current_accuracy(),
            }),
        })
    }

    /// Create status query message
    pub fn create_status_query(&self, target: NodeId) -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: self.get_current_time(),
                priority: Priority::Regular,
                source: self.node_id,
                target,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::StatusQuery),
        }
    }

    /// Handle status query
    pub fn handle_status_query(&self, query: &Message) -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: self.get_current_time(),
                priority: Priority::Regular,
                source: self.node_id,
                target: query.header.source,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::StatusResponse {
                is_synced: self.synchronizer.get_status() == SyncStatus::Synced,
                current_offset_ms: self.synchronizer.get_current_offset_ms(),
                last_sync_time: self.get_current_time(),
                accuracy_ms: self.get_current_accuracy(),
            }),
        }
    }

    // Helper methods
    fn get_current_time(&self) -> OffsetDateTime {
        let current_ms = self.time_provider.uptime_ms();
        let adjusted_ms = self.synchronizer.get_adjusted_time(current_ms);
        OffsetDateTime::from_unix_timestamp(adjusted_ms as i64 / 1000)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    fn estimate_network_delay(&self, _target: NodeId) -> u32 {
        match self.node_id {
            NodeId::Cloud => 50,     // Cloud processing delay
            NodeId::Edge(_) => 20,   // Edge processing delay
            NodeId::Device(_) => 10, // Device processing delay
        }
    }

    fn get_precision_requirement(&self) -> u16 {
        match self.node_id {
            NodeId::Cloud => 1,      // 1ms
            NodeId::Edge(_) => 10,   // 10ms
            NodeId::Device(_) => 50, // 50ms
        }
    }

    fn get_current_accuracy(&self) -> u16 {
        match self.synchronizer.get_status() {
            SyncStatus::Synced => self.get_precision_requirement(),
            SyncStatus::Syncing => self.get_precision_requirement() * 2,
            _ => u16::MAX,
        }
    }

    // Public API
    pub fn get_sync_status(&self) -> SyncStatus {
        self.synchronizer.get_status()
    }

    pub fn get_adjusted_time(&self) -> u64 {
        let current_time = self.time_provider.uptime_ms();
        self.synchronizer.get_adjusted_time(current_time)
    }

    pub fn get_current_offset_ms(&self) -> i64 {
        self.synchronizer.get_current_offset_ms()
    }

    pub fn needs_sync(&self) -> bool {
        let current_time = self.time_provider.uptime_ms();
        self.synchronizer.needs_sync(current_time)
    }

    pub fn reset_sync(&mut self) {
        self.synchronizer.reset();
        self.pending_requests.clear();
        self.sequence_counter = 0;
    }

    /// Clean up expired pending requests
    pub fn cleanup_expired_requests(&mut self) {
        let current_time = self.time_provider.uptime_ms();
        let timeout_ms = 10000; // 10 second timeout

        self.pending_requests
            .retain(|_, &mut req_time| current_time.saturating_sub(req_time) < timeout_ms);
    }
}

pub struct TimeSyncCoordinator<T: TimeProvider> {
    services: BTreeMap<NodeId, TimeSyncService<T>>,
}

impl<T: TimeProvider + Clone> TimeSyncCoordinator<T> {
    pub fn new() -> Self {
        Self {
            services: BTreeMap::new(),
        }
    }

    pub fn add_service(&mut self, node_id: NodeId, service: TimeSyncService<T>) {
        self.services.insert(node_id, service);
    }

    pub fn get_service(&mut self, node_id: NodeId) -> Option<&mut TimeSyncService<T>> {
        self.services.get_mut(&node_id)
    }

    /// Handle time synchronization message
    pub fn handle_time_sync_message(&mut self, msg: &Message) -> Option<Message> {
        if let MessagePayload::TimeSync(payload) = &msg.payload {
            if let Some(service) = self.services.get_mut(&msg.header.target) {
                match payload {
                    TimeSyncPayload::Request { .. } => service.handle_sync_request(msg).ok(),
                    TimeSyncPayload::Response { .. } => {
                        service.handle_sync_response(msg).ok();
                        None
                    }
                    TimeSyncPayload::StatusQuery => Some(service.handle_status_query(msg)),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get network status statistics
    pub fn get_network_status(&self) -> NetworkStatus {
        let mut status = NetworkStatus {
            total_nodes: self.services.len(),
            synced_nodes: 0,
            failed_nodes: 0,
            average_accuracy_ms: 0.0,
        };

        let mut total_accuracy = 0u32;
        let mut accuracy_count = 0;

        for service in self.services.values() {
            match service.get_sync_status() {
                SyncStatus::Synced => status.synced_nodes += 1,
                SyncStatus::Failed => status.failed_nodes += 1,
                _ => {}
            }

            let accuracy = service.get_current_accuracy();
            if accuracy != u16::MAX {
                total_accuracy += accuracy as u32;
                accuracy_count += 1;
            }
        }

        if accuracy_count > 0 {
            status.average_accuracy_ms = total_accuracy as f32 / accuracy_count as f32;
        }

        status
    }
}

#[derive(Debug, Clone)]
pub struct NetworkStatus {
    pub total_nodes: usize,
    pub synced_nodes: usize,
    pub failed_nodes: usize,
    pub average_accuracy_ms: f32,
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    #[derive(Clone)]
    struct MockTimeProvider {
        current_time: u64,
    }

    impl MockTimeProvider {
        fn new(initial_time: u64) -> Self {
            Self {
                current_time: initial_time,
            }
        }

        fn advance_time(&mut self, ms: u64) {
            self.current_time += ms;
        }
    }

    impl TimeProvider for MockTimeProvider {
        fn uptime_ms(&self) -> u64 {
            self.current_time
        }
    }

    fn create_test_config() -> SyncConfig {
        SyncConfig {
            sync_interval_ms: 1000,
            max_drift_ms: 100,
            offset_history_size: 5,
            delay_compensation_threshold_ms: 50,
            max_retry_count: 3,
        }
    }

    #[test]
    fn test_service_creation() {
        let time_provider = MockTimeProvider::new(1000);
        let config = create_test_config();
        let service = TimeSyncService::new(time_provider, NodeId::Edge(1), config);

        assert_eq!(service.get_sync_status(), SyncStatus::Unsynced);
        assert_eq!(service.get_current_offset_ms(), 0);
        assert!(service.needs_sync());
    }

    #[test]
    fn test_create_sync_request() {
        let time_provider = MockTimeProvider::new(1000);
        let config = create_test_config();
        let mut service = TimeSyncService::new(time_provider, NodeId::Edge(1), config);

        let target = NodeId::Cloud;
        let request = service.create_sync_request(target).unwrap();

        assert_eq!(request.header.source, NodeId::Edge(1));
        assert_eq!(request.header.target, target);

        if let MessagePayload::TimeSync(TimeSyncPayload::Request { sequence, .. }) = request.payload
        {
            assert_eq!(sequence, 1);
            assert!(service.pending_requests.contains_key(&sequence));
        } else {
            panic!("Expected TimeSync Request payload");
        }
    }

    #[test]
    fn test_handle_sync_request() {
        let time_provider = MockTimeProvider::new(2000);
        let config = create_test_config();
        let mut service = TimeSyncService::new(time_provider, NodeId::Cloud, config);

        let request = Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                sequence: 1,
                send_time: OffsetDateTime::UNIX_EPOCH,
                precision_ms: 10,
            }),
        };

        let response = service.handle_sync_request(&request).unwrap();

        assert_eq!(response.header.source, NodeId::Cloud);
        assert_eq!(response.header.target, NodeId::Edge(1));

        if let MessagePayload::TimeSync(TimeSyncPayload::Response {
            request_sequence, ..
        }) = response.payload
        {
            assert_eq!(request_sequence, 1);
        } else {
            panic!("Expected TimeSync Response payload");
        }
    }

    #[test]
    fn test_handle_sync_response() {
        let time_provider = MockTimeProvider::new(3000);
        let config = create_test_config();
        let mut service = TimeSyncService::new(time_provider, NodeId::Edge(1), config);

        // First create a request to establish pending request
        let _request = service.create_sync_request(NodeId::Cloud).unwrap();

        let response = Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Response {
                request_sequence: 1,
                request_receive_time: OffsetDateTime::UNIX_EPOCH,
                response_send_time: OffsetDateTime::from_unix_timestamp(3).unwrap(),
                estimated_delay_ms: 25,
                accuracy_ms: 5,
            }),
        };

        let result = service.handle_sync_response(&response);
        assert!(result.is_ok());
        assert_eq!(service.get_sync_status(), SyncStatus::Synced);
        assert!(!service.pending_requests.contains_key(&1));
    }

    #[test]
    fn test_create_time_broadcast() {
        let time_provider = MockTimeProvider::new(4000);
        let config = create_test_config();
        let edge_service =
            TimeSyncService::new(time_provider.clone(), NodeId::Edge(1), config.clone());
        let device_service =
            TimeSyncService::new(time_provider, NodeId::Device([1, 2, 3, 4, 5, 6]), config);

        // Edge node should be able to broadcast
        let broadcast = edge_service.create_time_broadcast();
        assert!(broadcast.is_ok());

        // Device node should not be able to broadcast
        let broadcast = device_service.create_time_broadcast();
        assert!(broadcast.is_err());
    }

    #[test]
    fn test_status_query_and_response() {
        let time_provider = MockTimeProvider::new(5000);
        let config = create_test_config();
        let service = TimeSyncService::new(time_provider, NodeId::Edge(1), config);

        let query = service.create_status_query(NodeId::Cloud);
        assert_eq!(query.header.source, NodeId::Edge(1));
        assert_eq!(query.header.target, NodeId::Cloud);

        if let MessagePayload::TimeSync(TimeSyncPayload::StatusQuery) = query.payload {
            // Expected
        } else {
            panic!("Expected StatusQuery payload");
        }

        let response = service.handle_status_query(&query);
        if let MessagePayload::TimeSync(TimeSyncPayload::StatusResponse { is_synced, .. }) =
            response.payload
        {
            assert!(!is_synced); // Should not be synced initially
        } else {
            panic!("Expected StatusResponse payload");
        }
    }

    #[test]
    fn test_cleanup_expired_requests() {
        let mut time_provider = MockTimeProvider::new(1000);
        let config = create_test_config();
        let mut service = TimeSyncService::new(time_provider.clone(), NodeId::Edge(1), config);

        // Create some requests
        let _req1 = service.create_sync_request(NodeId::Cloud).unwrap();
        time_provider.advance_time(100);
        service.time_provider = time_provider.clone();
        let _req2 = service.create_sync_request(NodeId::Cloud).unwrap();

        assert_eq!(service.pending_requests.len(), 2);

        // Advance time significantly
        time_provider.advance_time(15000); // 15 seconds
        service.time_provider = time_provider;
        service.cleanup_expired_requests();

        // All requests should be expired and removed
        assert_eq!(service.pending_requests.len(), 0);
    }

    #[test]
    fn test_coordinator() {
        let time_provider = MockTimeProvider::new(6000);
        let config = create_test_config();
        let mut coordinator = TimeSyncCoordinator::new();

        let cloud_service =
            TimeSyncService::new(time_provider.clone(), NodeId::Cloud, config.clone());
        let edge_service = TimeSyncService::new(time_provider, NodeId::Edge(1), config);

        coordinator.add_service(NodeId::Cloud, cloud_service);
        coordinator.add_service(NodeId::Edge(1), edge_service);

        // Test status
        let status = coordinator.get_network_status();
        assert_eq!(status.total_nodes, 2);
        assert_eq!(status.synced_nodes, 0);
        assert_eq!(status.failed_nodes, 0);
    }

    #[test]
    fn test_coordinator_message_handling() {
        let time_provider = MockTimeProvider::new(7000);
        let config = create_test_config();
        let mut coordinator = TimeSyncCoordinator::new();

        let cloud_service =
            TimeSyncService::new(time_provider.clone(), NodeId::Cloud, config.clone());
        let edge_service = TimeSyncService::new(time_provider, NodeId::Edge(1), config);

        coordinator.add_service(NodeId::Cloud, cloud_service);
        coordinator.add_service(NodeId::Edge(1), edge_service);

        let request = Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                sequence: 1,
                send_time: OffsetDateTime::UNIX_EPOCH,
                precision_ms: 10,
            }),
        };

        let response = coordinator.handle_time_sync_message(&request);
        assert!(response.is_some());

        if let Some(resp) = response {
            assert_eq!(resp.header.source, NodeId::Cloud);
            assert_eq!(resp.header.target, NodeId::Edge(1));
        }
    }

    #[test]
    fn test_reset_sync() {
        let time_provider = MockTimeProvider::new(8000);
        let config = create_test_config();
        let mut service = TimeSyncService::new(time_provider, NodeId::Edge(1), config);

        // Create a request and simulate some state
        let _req = service.create_sync_request(NodeId::Cloud).unwrap();
        assert!(!service.pending_requests.is_empty());
        assert_ne!(service.sequence_counter, 0);

        // Reset
        service.reset_sync();

        assert!(service.pending_requests.is_empty());
        assert_eq!(service.sequence_counter, 0);
        assert_eq!(service.get_sync_status(), SyncStatus::Unsynced);
        assert_eq!(service.get_current_offset_ms(), 0);
    }
}
