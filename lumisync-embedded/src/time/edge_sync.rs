use lumisync_api::{NodeId, SyncConfig, SyncStatus, TimeSyncService};
use time::OffsetDateTime;

use crate::{Error, Result};

use super::provider::EmbeddedTimeProvider;

pub struct EdgeTimeSync {
    /// Base time synchronization service
    sync_service: TimeSyncService<EmbeddedTimeProvider>,
    /// Edge node ID
    pub edge_id: u8,
    /// Last cloud sync time
    pub last_cloud_sync: Option<u64>,
    /// Cloud sync interval (milliseconds)
    pub cloud_sync_interval_ms: u64,
}

impl EdgeTimeSync {
    /// Default cloud sync interval: 10 seconds
    pub const DEFAULT_CLOUD_SYNC_INTERVAL_MS: u64 = 10_000;

    pub fn new(edge_id: u8) -> Self {
        let time_provider = EmbeddedTimeProvider::new();
        let node_id = NodeId::Edge(edge_id);
        let config = SyncConfig {
            sync_interval_ms: Self::DEFAULT_CLOUD_SYNC_INTERVAL_MS,
            max_drift_ms: 100,
            offset_history_size: 5,
            delay_compensation_threshold_ms: 50,
            max_retry_count: 3,
        };

        let sync_service = TimeSyncService::new(time_provider, node_id, config);

        Self {
            sync_service,
            edge_id,
            last_cloud_sync: None,
            cloud_sync_interval_ms: Self::DEFAULT_CLOUD_SYNC_INTERVAL_MS,
        }
    }

    /// Create edge time synchronization manager with custom configuration
    pub fn with_config(edge_id: u8, config: SyncConfig) -> Self {
        let time_provider = EmbeddedTimeProvider::new();
        let node_id = NodeId::Edge(edge_id);
        let cloud_sync_interval_ms = config.sync_interval_ms;
        let sync_service = TimeSyncService::new(time_provider, node_id, config);

        Self {
            sync_service,
            edge_id,
            last_cloud_sync: None,
            cloud_sync_interval_ms,
        }
    }

    /// Check if synchronization with cloud server is needed
    pub fn needs_cloud_sync(&self) -> bool {
        self.sync_service.needs_sync()
    }

    /// Create cloud synchronization request
    pub fn create_cloud_sync_request(&mut self) -> Result<lumisync_api::Message> {
        self.sync_service
            .create_sync_request(NodeId::Cloud)
            .map_err(|_| Error::InvalidCommand)
    }

    /// Handle cloud synchronization response
    pub fn handle_cloud_sync_response(&mut self, response: &lumisync_api::Message) -> Result<()> {
        self.sync_service
            .handle_sync_response(response)
            .map_err(|_| Error::InvalidCommand)?;

        self.last_cloud_sync = Some(self.sync_service.get_adjusted_time());
        Ok(())
    }

    /// Create time broadcast to devices
    pub fn create_time_broadcast(&self) -> Result<lumisync_api::Message> {
        self.sync_service
            .create_time_broadcast()
            .map_err(|_| Error::InvalidCommand)
    }

    /// Handle device synchronization request
    pub fn handle_device_sync_request(
        &mut self,
        request: &lumisync_api::Message,
    ) -> Result<lumisync_api::Message> {
        self.sync_service
            .handle_sync_request(request)
            .map_err(|_| Error::InvalidCommand)
    }

    /// Get synchronization status
    pub fn get_sync_status(&self) -> SyncStatus {
        self.sync_service.get_sync_status()
    }

    /// Get current time
    pub fn get_current_time(&self) -> OffsetDateTime {
        let adjusted_time = self.sync_service.get_adjusted_time();
        OffsetDateTime::from_unix_timestamp((adjusted_time / 1000) as i64)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    /// Get time offset
    pub fn get_time_offset_ms(&self) -> i64 {
        self.sync_service.get_current_offset_ms()
    }

    /// Reset synchronization state
    pub fn reset(&mut self) {
        self.sync_service.reset_sync();
        self.last_cloud_sync = None;
    }
}
