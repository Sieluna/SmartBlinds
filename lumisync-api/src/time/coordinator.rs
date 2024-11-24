use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::message::*;
use crate::uuid::{DeviceBasedUuidGenerator, UuidGenerator};

use super::{NetworkStatus, SyncStatus, TimeProvider, TimeSyncService};

pub struct TimeSyncCoordinator<T: TimeProvider, U: UuidGenerator = DeviceBasedUuidGenerator> {
    services: BTreeMap<NodeId, TimeSyncService<T, U>>,
}

impl<T: TimeProvider, U: UuidGenerator> TimeSyncCoordinator<T, U> {
    pub fn new() -> Self {
        Self {
            services: BTreeMap::new(),
        }
    }

    pub fn add_service(&mut self, node_id: NodeId, service: TimeSyncService<T, U>) {
        self.services.insert(node_id, service);
    }

    pub fn remove_service(&mut self, node_id: NodeId) -> Option<TimeSyncService<T, U>> {
        self.services.remove(&node_id)
    }

    pub fn get_service(&mut self, node_id: NodeId) -> Option<&mut TimeSyncService<T, U>> {
        self.services.get_mut(&node_id)
    }

    pub fn get_service_immutable(&self, node_id: NodeId) -> Option<&TimeSyncService<T, U>> {
        self.services.get(&node_id)
    }

    /// Handle time sync message and route to appropriate service
    pub fn handle_time_sync_message(&mut self, msg: &Message) -> Option<Message> {
        if let MessagePayload::TimeSync(payload) = &msg.payload {
            if let Some(service) = self.services.get_mut(&msg.header.target) {
                match payload {
                    TimeSyncPayload::Request { .. } => service.handle_sync_request(msg).ok(),
                    TimeSyncPayload::Response { .. } => {
                        let _ = service.handle_sync_response(msg);
                        None
                    }
                    TimeSyncPayload::StatusQuery => service.handle_status_query(msg).ok(),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Broadcast message to all services
    pub fn broadcast_message(
        &mut self,
        msg: &Message,
    ) -> Vec<(NodeId, Result<Message, super::SyncError>)> {
        let mut responses = Vec::new();

        for (node_id, service) in &mut self.services {
            if let MessagePayload::TimeSync(payload) = &msg.payload {
                let result = match payload {
                    TimeSyncPayload::Request { .. } => service.handle_sync_request(msg),
                    TimeSyncPayload::StatusQuery => service.handle_status_query(msg),
                    _ => continue,
                };
                responses.push((*node_id, result));
            }
        }

        responses
    }

    /// Get network statistics
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
                SyncStatus::Failed { .. } => status.failed_nodes += 1,
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

    /// Get list of all managed node IDs
    pub fn get_node_ids(&self) -> Vec<NodeId> {
        self.services.keys().copied().collect()
    }

    /// Get count of services
    pub fn service_count(&self) -> usize {
        self.services.len()
    }

    /// Reset all services
    pub fn reset_all(&mut self) {
        for service in self.services.values_mut() {
            service.reset_sync();
        }
    }
}

impl<T: TimeProvider, U: UuidGenerator> Default for TimeSyncCoordinator<T, U> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::time::{SyncConfig, TimeSyncService};
    use crate::uuid::DeviceBasedUuidGenerator;

    use super::*;

    #[derive(Clone)]
    struct MockTimeProvider {
        uptime_ms: u64,
    }

    impl MockTimeProvider {
        fn new(uptime: u64) -> Self {
            Self { uptime_ms: uptime }
        }
    }

    impl TimeProvider for MockTimeProvider {
        fn monotonic_time_ms(&self) -> u64 {
            self.uptime_ms
        }
    }

    #[test]
    fn test_coordinator_comprehensive_operations() {
        let mut coordinator = TimeSyncCoordinator::new();
        let provider = MockTimeProvider::new(1000);
        let config = SyncConfig::default();
        let uuid_gen = DeviceBasedUuidGenerator::new([1, 2, 3, 4, 5, 6]);
        let service = TimeSyncService::new(provider, NodeId::Edge(1), config, uuid_gen);

        // Service lifecycle management
        coordinator.add_service(NodeId::Edge(1), service);
        assert_eq!(coordinator.service_count(), 1);
        assert!(coordinator.get_service(NodeId::Edge(1)).is_some());
        assert!(coordinator.get_service_immutable(NodeId::Edge(1)).is_some());

        let node_ids = coordinator.get_node_ids();
        assert_eq!(node_ids.len(), 1);
        assert_eq!(node_ids[0], NodeId::Edge(1));

        let removed = coordinator.remove_service(NodeId::Edge(1));
        assert!(removed.is_some());
        assert_eq!(coordinator.service_count(), 0);
        assert!(coordinator.get_service(NodeId::Edge(2)).is_none());
    }
}
