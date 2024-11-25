use std::collections::HashSet;
use std::sync::Arc;

use lumisync_api::handler::{MessageError, MessageHandler, PayloadType};
use lumisync_api::message::*;
use lumisync_api::time::{SyncConfig, TimeSyncCoordinator, TimeSyncService};
use lumisync_api::uuid::RandomUuidGenerator;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::configs::Storage;
use crate::services::message_service::SystemTimeProvider;

type CloudTimeSyncService = TimeSyncService<SystemTimeProvider, RandomUuidGenerator>;
type CloudTimeSyncCoordinator = TimeSyncCoordinator<SystemTimeProvider, RandomUuidGenerator>;

pub struct CloudTimeSyncHandler {
    /// Time synchronization coordinator
    coordinator: Arc<RwLock<CloudTimeSyncCoordinator>>,
    /// List of authorized Edge nodes
    authorized_edges: Arc<RwLock<HashSet<u8>>>,
    /// Local time synchronization service
    time_service: CloudTimeSyncService,
}

impl CloudTimeSyncHandler {
    pub fn new(
        coordinator: Arc<RwLock<CloudTimeSyncCoordinator>>,
        authorized_edges: HashSet<u8>,
        _storage: Option<Arc<Storage>>,
    ) -> Self {
        let config = SyncConfig {
            sync_interval_ms: 0,
            max_drift_ms: 0,
            offset_history_size: 1,
            delay_threshold_ms: 2000,
            max_retry_count: 0,
            failure_cooldown_ms: 0,
        };

        let time_service = TimeSyncService::new(
            SystemTimeProvider,
            NodeId::Cloud,
            config,
            RandomUuidGenerator,
        );

        Self {
            coordinator,
            authorized_edges: Arc::new(RwLock::new(authorized_edges)),
            time_service,
        }
    }

    /// Add authorized Edge node
    pub async fn authorize_edge(&self, edge_id: u8) {
        let mut edges = self.authorized_edges.write().await;
        edges.insert(edge_id);
        info!(
            "Edge node {} has been authorized for time synchronization",
            edge_id
        );
    }

    /// Remove Edge node authorization
    pub async fn revoke_edge_authorization(&self, edge_id: u8) {
        let mut edges = self.authorized_edges.write().await;
        edges.remove(&edge_id);
        warn!(
            "Revoked time synchronization authorization for edge node {}",
            edge_id
        );
    }

    /// Handle time synchronization request
    async fn handle_time_sync_request(
        &mut self,
        message: Message,
    ) -> Result<Option<Message>, MessageError> {
        if let NodeId::Edge(edge_id) = message.header.source {
            // Check authorization
            let edges = self.authorized_edges.read().await;
            if !edges.contains(&edge_id) {
                warn!(
                    "Unauthorized edge node {} attempted time synchronization",
                    edge_id
                );
                return Err(MessageError::Unauthorized);
            }
            drop(edges);

            // Process sync request
            match self.time_service.handle_sync_request(&message) {
                Ok(response) => {
                    debug!("Provided authoritative time sync for edge node {}", edge_id);
                    Ok(Some(response))
                }
                Err(e) => {
                    error!(
                        "Failed to process time sync request from edge node {}: {}",
                        edge_id, e
                    );
                    Err(MessageError::InternalError(format!(
                        "Sync processing failed: {}",
                        e
                    )))
                }
            }
        } else {
            warn!(
                "Time sync request from non-edge node: {:?}",
                message.header.source
            );
            Err(MessageError::Unauthorized)
        }
    }

    /// Handle status query
    async fn handle_status_query(
        &mut self,
        message: Message,
    ) -> Result<Option<Message>, MessageError> {
        if let NodeId::Edge(edge_id) = message.header.source {
            // Check authorization
            let edges = self.authorized_edges.read().await;
            if !edges.contains(&edge_id) {
                return Err(MessageError::Unauthorized);
            }
            drop(edges);

            match self.time_service.handle_status_query(&message) {
                Ok(response) => {
                    debug!("Provided time sync status for edge node {}", edge_id);
                    Ok(Some(response))
                }
                Err(e) => {
                    error!(
                        "Failed to process status query from edge node {}: {}",
                        edge_id, e
                    );
                    Err(MessageError::InternalError(format!(
                        "Status query failed: {}",
                        e
                    )))
                }
            }
        } else {
            warn!(
                "Status query from non-edge node: {:?}",
                message.header.source
            );
            Err(MessageError::Unauthorized)
        }
    }

    /// Get synchronization statistics
    pub async fn get_sync_stats(&self) -> TimeSyncStats {
        let coordinator = self.coordinator.read().await;
        let network_status = coordinator.get_network_status();

        TimeSyncStats {
            total_nodes: network_status.total_nodes,
            synced_nodes: network_status.synced_nodes,
            failed_nodes: network_status.failed_nodes,
            average_accuracy_ms: network_status.average_accuracy_ms,
            authorized_edges: self.authorized_edges.read().await.len(),
        }
    }
}

impl MessageHandler for CloudTimeSyncHandler {
    fn handle_message(&mut self, message: Message) -> Result<Option<Message>, MessageError> {
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                match &message.payload {
                    MessagePayload::TimeSync(sync_payload) => match sync_payload {
                        TimeSyncPayload::Request { .. } => {
                            self.handle_time_sync_request(message).await
                        }
                        TimeSyncPayload::StatusQuery => self.handle_status_query(message).await,
                        _ => {
                            debug!("Cloud ignoring time sync message type: {:?}", sync_payload);
                            Ok(None)
                        }
                    },
                    _ => Ok(None),
                }
            })
        })
    }

    fn supported_payloads(&self) -> Vec<PayloadType> {
        vec![PayloadType::TimeSync]
    }

    fn node_id(&self) -> NodeId {
        NodeId::Cloud
    }

    fn name(&self) -> &'static str {
        "CloudTimeSyncHandler"
    }
}

#[derive(Debug, Clone)]
pub struct TimeSyncStats {
    pub total_nodes: usize,
    pub synced_nodes: usize,
    pub failed_nodes: usize,
    pub average_accuracy_ms: f32,
    pub authorized_edges: usize,
}

impl TimeSyncStats {
    pub fn sync_success_rate(&self) -> f64 {
        if self.total_nodes == 0 {
            0.0
        } else {
            self.synced_nodes as f64 / self.total_nodes as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use lumisync_api::message::{MessageHeader, Priority, TimeSyncPayload};
    use lumisync_api::time::TimeSyncCoordinator;
    use lumisync_api::uuid::RandomUuidGenerator;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::services::message_service::SystemTimeProvider;

    use super::*;

    #[tokio::test]
    async fn test_authorized_edge_sync_request() {
        let coordinator = Arc::new(RwLock::new(TimeSyncCoordinator::<
            SystemTimeProvider,
            RandomUuidGenerator,
        >::new()));
        let mut authorized_edges = HashSet::new();
        authorized_edges.insert(1u8);

        let mut handler = CloudTimeSyncHandler::new(coordinator, authorized_edges, None);

        let request = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                sequence: 1,
                send_time: Some(OffsetDateTime::now_utc()),
                precision_ms: 10,
            }),
        };

        let result = handler.handle_time_sync_request(request).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_unauthorized_edge_sync_request() {
        let coordinator = Arc::new(RwLock::new(TimeSyncCoordinator::<
            SystemTimeProvider,
            RandomUuidGenerator,
        >::new()));
        let authorized_edges = HashSet::new();

        let mut handler = CloudTimeSyncHandler::new(coordinator, authorized_edges, None);

        let request = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                sequence: 1,
                send_time: Some(OffsetDateTime::now_utc()),
                precision_ms: 10,
            }),
        };

        let result = handler.handle_time_sync_request(request).await;
        assert!(matches!(result, Err(MessageError::Unauthorized)));
    }
}
