use lumisync_api::message::*;
use lumisync_api::time::{SyncConfig, SyncStatus, TimeProvider, TimeSyncService};
use lumisync_api::uuid::{DeviceBasedUuidGenerator, UuidGenerator};
use time::OffsetDateTime;

use crate::{Error, Result};

use super::provider::EmbeddedTimeProvider;

#[derive(Debug, Clone, Default)]
pub struct EdgeSyncStats {
    pub cloud_syncs: u16,      // Cloud sync attempts
    pub cloud_successes: u16,  // Successful cloud syncs
    pub device_requests: u16,  // Device sync requests handled
    pub broadcasts_sent: u16,  // Time broadcasts sent
    pub last_accuracy_ms: u16, // Last cloud accuracy
}

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
    /// Number of successful cloud syncs
    pub cloud_sync_count: u32,
    /// Number of device sync requests handled
    pub device_sync_requests: u32,
    /// Edge synchronization statistics
    pub stats: EdgeSyncStats,
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
            cloud_sync_count: 0,
            device_sync_requests: 0,
            stats: EdgeSyncStats::default(),
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
            cloud_sync_count: 0,
            device_sync_requests: 0,
            stats: EdgeSyncStats::default(),
        }
    }

    /// Check if cloud synchronization is needed
    pub fn needs_cloud_sync(&self) -> bool {
        self.sync_service.needs_sync()
    }

    /// Create cloud synchronization request
    pub fn create_cloud_sync_request(&mut self) -> Result<Message> {
        self.stats.cloud_syncs = self.stats.cloud_syncs.saturating_add(1);
        self.sync_service
            .create_sync_request(NodeId::Cloud)
            .map_err(|_| Error::InvalidCommand)
    }

    /// Handle cloud synchronization response
    pub fn handle_cloud_sync_response(&mut self, response: &Message) -> Result<()> {
        let result = self
            .sync_service
            .handle_sync_response(response)
            .map_err(|_| Error::InvalidCommand);

        match result {
            Ok(_) => {
                self.last_cloud_sync = Some(self.time_provider.monotonic_time_ms());
                self.stats.cloud_successes = self.stats.cloud_successes.saturating_add(1);

                // Extract accuracy from response
                if let MessagePayload::TimeSync(TimeSyncPayload::Response { accuracy_ms, .. }) =
                    &response.payload
                {
                    self.stats.last_accuracy_ms = *accuracy_ms;
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Create time broadcast to devices
    pub fn create_time_broadcast(&mut self) -> Result<Message> {
        let current_uptime = self.time_provider.monotonic_time_ms();
        let current_time = self
            .sync_service
            .get_network_time(current_uptime)
            .map_err(|_| Error::InvalidCommand)?;
        let current_offset = self.sync_service.get_current_offset_ms();
        let accuracy = self.sync_service.get_current_accuracy();

        self.stats.broadcasts_sent = self.stats.broadcasts_sent.saturating_add(1);

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
    pub fn handle_device_sync_request(&mut self, request: &Message) -> Result<Message> {
        self.stats.device_requests = self.stats.device_requests.saturating_add(1);
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

    /// Get time since last cloud sync in milliseconds
    pub fn time_since_last_cloud_sync_ms(&self) -> Option<u64> {
        self.last_cloud_sync.map(|last_sync| {
            let current_time = self.time_provider.monotonic_time_ms();
            current_time.saturating_sub(last_sync)
        })
    }

    /// Get cloud sync success rate (0.0 to 1.0)
    pub fn get_cloud_sync_quality(&self) -> f32 {
        if self.stats.cloud_syncs > 0 {
            self.stats.cloud_successes as f32 / self.stats.cloud_syncs as f32
        } else {
            0.0
        }
    }

    /// Check if cloud sync interval has elapsed
    pub fn should_sync_with_cloud(&self) -> bool {
        if let Some(last_sync) = self.last_cloud_sync {
            let current_time = self.time_provider.monotonic_time_ms();
            current_time.saturating_sub(last_sync) >= self.cloud_sync_interval_ms
        } else {
            true // No sync yet, should sync immediately
        }
    }

    /// Reset synchronization state
    pub fn reset(&mut self) {
        self.sync_service.reset_sync();
        self.last_cloud_sync = None;
        // Keep stats for monitoring
    }

    /// Reset statistics (for testing or periodic cleanup)
    pub fn reset_stats(&mut self) {
        self.stats = EdgeSyncStats::default();
    }

    /// Get current uptime in milliseconds
    pub fn get_uptime_ms(&self) -> u64 {
        self.time_provider.monotonic_time_ms()
    }
}
