use lumisync_api::time::{SyncConfig, SyncStatus, TimeProvider, TimeSyncService};
use lumisync_api::uuid::{DeviceBasedUuidGenerator, UuidGenerator};
use lumisync_api::{Message, MessageHeader, MessagePayload, NodeId, Priority, TimeSyncPayload};
use time::OffsetDateTime;

use crate::{Error, Result};

use super::provider::EmbeddedTimeProvider;

pub struct EdgeTimeSync {
    /// Base time synchronization service
    sync_service: TimeSyncService<EmbeddedTimeProvider, DeviceBasedUuidGenerator>,
    /// Edge node ID
    pub edge_id: u8,
    /// Last cloud sync time
    pub last_cloud_sync: Option<u64>,
    /// Cloud sync interval in milliseconds
    pub cloud_sync_interval_ms: u64,
    /// Time provider for getting current time
    time_provider: EmbeddedTimeProvider,
    /// UUID generator for creating message IDs
    uuid_generator: DeviceBasedUuidGenerator,
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
            delay_threshold_ms: 50,
            max_retry_count: 3,
            failure_cooldown_ms: 30000,
        };

        let uuid_generator = DeviceBasedUuidGenerator::new([0x00, 0x00, 0x00, 0x00, 0x00, edge_id]);
        let sync_service = TimeSyncService::new(
            time_provider.clone(),
            node_id,
            config,
            uuid_generator.clone(),
        );

        Self {
            sync_service,
            edge_id,
            last_cloud_sync: None,
            cloud_sync_interval_ms: Self::DEFAULT_CLOUD_SYNC_INTERVAL_MS,
            time_provider,
            uuid_generator,
        }
    }

    /// Create edge time sync manager with custom configuration
    pub fn with_config(edge_id: u8, config: SyncConfig) -> Self {
        let time_provider = EmbeddedTimeProvider::new();
        let node_id = NodeId::Edge(edge_id);
        let cloud_sync_interval_ms = config.sync_interval_ms;
        let uuid_generator = DeviceBasedUuidGenerator::new([0x00, 0x00, 0x00, 0x00, 0x00, edge_id]);
        let sync_service = TimeSyncService::new(
            time_provider.clone(),
            node_id,
            config,
            uuid_generator.clone(),
        );

        Self {
            sync_service,
            edge_id,
            last_cloud_sync: None,
            cloud_sync_interval_ms,
            time_provider,
            uuid_generator,
        }
    }

    /// Check if cloud synchronization is needed
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

        let current_uptime = self.time_provider.monotonic_time_ms();
        self.last_cloud_sync = Some(current_uptime);
        Ok(())
    }

    /// Create time broadcast to devices
    pub fn create_time_broadcast(&self) -> Result<lumisync_api::Message> {
        let current_uptime = self.time_provider.monotonic_time_ms();
        let current_time = self
            .sync_service
            .get_network_time(current_uptime)
            .map_err(|_| Error::InvalidCommand)?;
        let current_offset = self.sync_service.get_current_offset_ms();
        let accuracy = self.sync_service.get_current_accuracy();

        Ok(Message {
            header: MessageHeader {
                id: self.uuid_generator.generate(),
                timestamp: current_time,
                priority: Priority::Regular,
                source: NodeId::Edge(self.edge_id),
                target: NodeId::Any,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Broadcast {
                timestamp: current_time,
                offset_ms: current_offset,
                accuracy_ms: accuracy,
            }),
        })
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
        let current_uptime = self.time_provider.monotonic_time_ms();
        self.sync_service
            .get_network_time(current_uptime)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    /// Get time offset in milliseconds
    pub fn get_time_offset_ms(&self) -> i64 {
        self.sync_service.get_current_offset_ms()
    }

    /// Reset synchronization state
    pub fn reset(&mut self) {
        self.sync_service.reset_sync();
        self.last_cloud_sync = None;
    }
}
