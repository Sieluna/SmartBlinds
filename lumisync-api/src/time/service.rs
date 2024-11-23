use alloc::collections::BTreeMap;

use time::OffsetDateTime;

use crate::message::*;
use crate::uuid::{DeviceBasedUuidGenerator, UuidGenerator};

use super::{SyncConfig, SyncError, SyncStatus, TimeProvider, TimeSynchronizer};

pub struct TimeSyncService<T: TimeProvider, U: UuidGenerator = DeviceBasedUuidGenerator> {
    time_provider: T,
    synchronizer: TimeSynchronizer,
    sequence_counter: u32,
    pending_requests: BTreeMap<u32, u64>, // sequence -> request_uptime
    node_id: NodeId,
    uuid_generator: U,
}

impl<T: TimeProvider, U: UuidGenerator> TimeSyncService<T, U> {
    pub fn new(time_provider: T, node_id: NodeId, config: SyncConfig, uuid_generator: U) -> Self {
        Self {
            time_provider,
            synchronizer: TimeSynchronizer::new(node_id, config),
            sequence_counter: 0,
            pending_requests: BTreeMap::new(),
            node_id,
            uuid_generator,
        }
    }

    /// Create time synchronization request
    pub fn create_sync_request(&mut self, target: NodeId) -> Result<Message, SyncError> {
        let current_uptime = self.time_provider.monotonic_time_ms();

        self.synchronizer.update_status(current_uptime);
        self.cleanup_expired_requests(current_uptime);

        self.sequence_counter = self.sequence_counter.wrapping_add(1);
        let sequence = self.sequence_counter;

        self.pending_requests.insert(sequence, current_uptime);

        // For first-time sync, send None as send_time to avoid drift errors
        let send_time = if matches!(self.synchronizer.get_status(), SyncStatus::Synced) {
            self.get_network_time(current_uptime).ok()
        } else {
            None
        };

        Ok(Message {
            header: MessageHeader {
                id: self.uuid_generator.generate(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: self.node_id,
                target,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                sequence,
                send_time,
                precision_ms: self.get_precision_requirement(),
            }),
        })
    }

    /// Handle sync request and create response
    pub fn handle_sync_request(&mut self, request: &Message) -> Result<Message, SyncError> {
        if let MessagePayload::TimeSync(TimeSyncPayload::Request { sequence, .. }) =
            &request.payload
        {
            let current_uptime = self.time_provider.monotonic_time_ms();

            Ok(Message {
                header: MessageHeader {
                    id: self.uuid_generator.generate(),
                    timestamp: self.get_network_time(current_uptime)?,
                    priority: Priority::Regular,
                    source: self.node_id,
                    target: request.header.source,
                },
                payload: MessagePayload::TimeSync(TimeSyncPayload::Response {
                    request_sequence: *sequence,
                    request_receive_time: self.get_network_time(current_uptime)?,
                    response_send_time: self.get_network_time(current_uptime)?,
                    estimated_delay_ms: self.estimate_network_delay(request.header.source),
                    accuracy_ms: self.get_current_accuracy(),
                }),
            })
        } else {
            Err(SyncError::InvalidTimestamp)
        }
    }

    /// Handle sync response and update synchronizer
    pub fn handle_sync_response(&mut self, response: &Message) -> Result<(), SyncError> {
        if let MessagePayload::TimeSync(TimeSyncPayload::Response {
            request_sequence,
            response_send_time,
            ..
        }) = &response.payload
        {
            if let Some(request_uptime) = self.pending_requests.remove(request_sequence) {
                let current_uptime = self.time_provider.monotonic_time_ms();
                let response_network_time = response_send_time.unix_timestamp() as u64 * 1000
                    + (response_send_time.nanosecond() / 1_000_000) as u64;

                self.synchronizer.handle_sync_response(
                    request_uptime,
                    response_network_time,
                    current_uptime,
                )
            } else {
                Err(SyncError::InvalidTimestamp)
            }
        } else {
            Err(SyncError::InvalidTimestamp)
        }
    }

    /// Create status query
    pub fn create_status_query(&self, target: NodeId) -> Result<Message, SyncError> {
        let current_uptime = self.time_provider.monotonic_time_ms();

        Ok(Message {
            header: MessageHeader {
                id: self.uuid_generator.generate(),
                timestamp: self.get_network_time(current_uptime)?,
                priority: Priority::Regular,
                source: self.node_id,
                target,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::StatusQuery),
        })
    }

    /// Handle status query
    pub fn handle_status_query(&self, query: &Message) -> Result<Message, SyncError> {
        let current_uptime = self.time_provider.monotonic_time_ms();

        Ok(Message {
            header: MessageHeader {
                id: self.uuid_generator.generate(),
                timestamp: self.get_network_time(current_uptime)?,
                priority: Priority::Regular,
                source: self.node_id,
                target: query.header.source,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::StatusResponse {
                is_synced: matches!(self.synchronizer.get_status(), SyncStatus::Synced),
                current_offset_ms: self.synchronizer.get_current_offset_ms(),
                last_sync_time: self.get_network_time(current_uptime)?,
                accuracy_ms: self.get_current_accuracy(),
            }),
        })
    }

    /// Convert uptime to network time with explicit error handling
    pub fn get_network_time(&self, uptime: u64) -> Result<OffsetDateTime, SyncError> {
        if let Some(wall_time) = self.time_provider.wall_clock_time() {
            return Ok(wall_time);
        }

        match self.node_id {
            NodeId::Edge(_) | NodeId::Device(_) => {
                // Try to use synchronized time first
                if let Ok(network_time_ms) = self.synchronizer.uptime_to_network_time(uptime) {
                    let timestamp_secs = (network_time_ms / 1000) as i64;
                    let timestamp_nanos = ((network_time_ms % 1000) as u32) * 1_000_000;

                    OffsetDateTime::from_unix_timestamp(timestamp_secs)
                        .and_then(|dt| dt.replace_nanosecond(timestamp_nanos))
                        .map_err(|_| SyncError::InvalidTimestamp)
                } else {
                    Err(SyncError::NotSynchronized)
                }
            }
            _ => Err(SyncError::NotSynchronized),
        }
    }

    fn estimate_network_delay(&self, target: NodeId) -> u32 {
        match (self.node_id, target) {
            (NodeId::Cloud, NodeId::Edge(_)) | (NodeId::Edge(_), NodeId::Cloud) => 50,
            (NodeId::Edge(_), NodeId::Device(_)) | (NodeId::Device(_), NodeId::Edge(_)) => 20,
            _ => 30,
        }
    }

    fn get_precision_requirement(&self) -> u16 {
        match self.node_id {
            NodeId::Cloud => 1,
            NodeId::Edge(_) => 10,
            NodeId::Device(_) => 50,
            NodeId::Any => 100,
        }
    }

    pub fn get_current_accuracy(&self) -> u16 {
        match self.synchronizer.get_status() {
            SyncStatus::Synced => self.get_precision_requirement(),
            _ => u16::MAX,
        }
    }

    fn cleanup_expired_requests(&mut self, current_uptime: u64) {
        let timeout_ms = 10000; // 10 seconds
        self.pending_requests
            .retain(|_, req_uptime| current_uptime.saturating_sub(*req_uptime) < timeout_ms);
    }

    // Public getters
    pub fn get_sync_status(&self) -> SyncStatus {
        self.synchronizer.get_status()
    }

    pub fn get_current_offset_ms(&self) -> i64 {
        self.synchronizer.get_current_offset_ms()
    }

    pub fn needs_sync(&self) -> bool {
        let current_uptime = self.time_provider.monotonic_time_ms();
        self.synchronizer.needs_sync(current_uptime)
    }

    pub fn reset_sync(&mut self) {
        self.synchronizer.reset();
        self.pending_requests.clear();
        self.sequence_counter = 0;
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn pending_request_count(&self) -> usize {
        self.pending_requests.len()
    }

    #[cfg(test)]
    pub fn get_synchronizer_mut(&mut self) -> &mut TimeSynchronizer {
        &mut self.synchronizer
    }

    #[cfg(test)]
    pub fn set_sequence_counter(&mut self, counter: u32) {
        self.sequence_counter = counter;
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::uuid::DeviceBasedUuidGenerator;

    use super::*;

    #[derive(Clone)]
    struct MockTimeProvider {
        uptime_ms: u64,
        has_wall_clock: bool,
    }

    impl MockTimeProvider {
        fn new(uptime: u64) -> Self {
            Self {
                uptime_ms: uptime,
                has_wall_clock: false,
            }
        }

        fn with_wall_clock(uptime: u64) -> Self {
            Self {
                uptime_ms: uptime,
                has_wall_clock: true,
            }
        }
    }

    impl TimeProvider for MockTimeProvider {
        fn monotonic_time_ms(&self) -> u64 {
            self.uptime_ms
        }

        fn wall_clock_time(&self) -> Option<OffsetDateTime> {
            if self.has_wall_clock {
                Some(OffsetDateTime::now_utc())
            } else {
                None
            }
        }
    }

    fn create_test_config() -> SyncConfig {
        SyncConfig {
            sync_interval_ms: 1000,
            max_drift_ms: 100,
            offset_history_size: 5,
            delay_threshold_ms: 50,
            max_retry_count: 3,
            failure_cooldown_ms: 5000,
        }
    }

    #[test]
    fn test_complete_sync_workflow() {
        let config = create_test_config();
        let cloud_provider = MockTimeProvider::with_wall_clock(1000);
        let edge_provider = MockTimeProvider::new(1000);
        let device_provider = MockTimeProvider::new(1000);

        let cloud_uuid = DeviceBasedUuidGenerator::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        let edge_uuid = DeviceBasedUuidGenerator::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        let device_uuid = DeviceBasedUuidGenerator::new([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);

        let mut cloud_service =
            TimeSyncService::new(cloud_provider, NodeId::Cloud, config.clone(), cloud_uuid);
        let mut edge_service =
            TimeSyncService::new(edge_provider, NodeId::Edge(1), config.clone(), edge_uuid);
        let mut device_service = TimeSyncService::new(
            device_provider,
            NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
            config,
            device_uuid,
        );

        // Cloud can create requests and handle requests
        assert!(cloud_service.create_sync_request(NodeId::Edge(1)).is_ok());

        // Edge can create requests but device cannot respond to requests (no authoritative time)
        assert!(edge_service.create_sync_request(NodeId::Cloud).is_ok());
        assert!(device_service.create_sync_request(NodeId::Cloud).is_ok());

        // Test request-response cycle with UUID uniqueness
        let req1 = edge_service.create_sync_request(NodeId::Cloud).unwrap();
        let req2 = edge_service.create_sync_request(NodeId::Cloud).unwrap();
        assert_ne!(req1.header.id, req2.header.id);
        assert_eq!(
            &req1.header.id.as_bytes()[0..6],
            &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]
        );

        // Cloud handles edge request
        let response = cloud_service.handle_sync_request(&req1).unwrap();
        assert_eq!(response.header.target, NodeId::Edge(1));

        // Edge processes response
        let result = edge_service.handle_sync_response(&response);
        assert!(result.is_ok());

        // Status queries work after sync setup
        let status_query = edge_service.create_status_query(NodeId::Cloud).unwrap();
        let status_response = cloud_service.handle_status_query(&status_query).unwrap();
        assert!(matches!(
            status_response.payload,
            MessagePayload::TimeSync(TimeSyncPayload::StatusResponse { .. })
        ));
    }

    #[test]
    fn test_sync_edge_cases_and_failures() {
        let mut config = create_test_config();
        config.max_retry_count = 2;
        config.failure_cooldown_ms = 1000;

        let provider = MockTimeProvider::new(5000);
        let uuid_gen = DeviceBasedUuidGenerator::new([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let mut service = TimeSyncService::new(provider, NodeId::Edge(99), config, uuid_gen);

        // Create multiple requests to test sequence handling
        let req1 = service.create_sync_request(NodeId::Cloud).unwrap();
        let _req2 = service.create_sync_request(NodeId::Cloud).unwrap();
        assert_eq!(service.pending_request_count(), 2);

        // Test invalid response handling
        let invalid_msg = Message {
            header: MessageHeader {
                id: uuid::Uuid::new_v4(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: NodeId::Edge(99),
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::StatusQuery),
        };

        assert!(service.handle_sync_response(&invalid_msg).is_err());

        // Test response with wrong sequence
        let wrong_seq_response = Message {
            header: MessageHeader {
                id: uuid::Uuid::new_v4(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: NodeId::Edge(99),
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Response {
                request_sequence: 999,
                request_receive_time: OffsetDateTime::UNIX_EPOCH,
                response_send_time: OffsetDateTime::UNIX_EPOCH,
                estimated_delay_ms: 50,
                accuracy_ms: 10,
            }),
        };

        assert!(service.handle_sync_response(&wrong_seq_response).is_err());

        // Test valid response processing
        if let MessagePayload::TimeSync(TimeSyncPayload::Request { sequence, .. }) = &req1.payload {
            let valid_response = Message {
                header: MessageHeader {
                    id: uuid::Uuid::new_v4(),
                    timestamp: OffsetDateTime::UNIX_EPOCH,
                    priority: Priority::Regular,
                    source: NodeId::Cloud,
                    target: NodeId::Edge(99),
                },
                payload: MessagePayload::TimeSync(TimeSyncPayload::Response {
                    request_sequence: *sequence,
                    request_receive_time: OffsetDateTime::from_unix_timestamp(1_600_000_000)
                        .unwrap(),
                    response_send_time: OffsetDateTime::from_unix_timestamp(1_600_000_000)
                        .unwrap()
                        .checked_add(time::Duration::milliseconds(5100))
                        .unwrap(), // 5000 + 100ms offset
                    estimated_delay_ms: 50,
                    accuracy_ms: 10,
                }),
            };

            let result = service.handle_sync_response(&valid_response);
            assert!(result.is_ok());
        }

        // Test cleanup and state management
        service.reset_sync();
        assert_eq!(service.pending_request_count(), 0);
        assert_eq!(service.get_sync_status(), SyncStatus::Unsynced);
        assert!(service.needs_sync());
    }
}
