use lumisync_api::message::{Message, MessagePayload, NodeId, TimeSyncPayload};
use lumisync_api::time::TimeProvider;
use time::OffsetDateTime;

use crate::{Error, Result};

use super::provider::EmbeddedTimeProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceSyncState {
    /// Not synchronized
    Unsynced,
    /// Synchronized
    Synced,
    /// Sync expired (exceeded threshold time without receiving updates)
    Expired,
}

/// Simplified device sync metrics for embedded systems
#[derive(Debug, Clone, Default)]
pub struct DeviceSyncStats {
    pub sync_count: u16,       // Total sync attempts (u16 saves memory)
    pub success_count: u16,    // Successful syncs
    pub last_accuracy_ms: u16, // Last reported accuracy
}

#[derive(Debug)]
pub struct DeviceTimeSync {
    /// Base time provider
    time_provider: EmbeddedTimeProvider,
    /// Device MAC address
    pub device_mac: [u8; 6],
    /// Time offset in milliseconds
    pub time_offset_ms: i64,
    /// Last sync time in local timestamp
    pub last_sync_time: Option<u64>,
    /// Synchronization state
    pub sync_state: DeviceSyncState,
    /// Sync expiry threshold in milliseconds
    pub sync_expiry_threshold_ms: u64,
    /// Synchronization statistics
    pub stats: DeviceSyncStats,
}

impl DeviceTimeSync {
    /// Default sync expiry threshold: 30 seconds
    pub const DEFAULT_SYNC_EXPIRY_MS: u64 = 30_000;

    pub fn new(device_mac: [u8; 6]) -> Self {
        Self {
            time_provider: EmbeddedTimeProvider::new(),
            device_mac,
            time_offset_ms: 0,
            last_sync_time: None,
            sync_state: DeviceSyncState::Unsynced,
            sync_expiry_threshold_ms: Self::DEFAULT_SYNC_EXPIRY_MS,
            stats: DeviceSyncStats::default(),
        }
    }

    /// Create time synchronization manager with custom expiry threshold
    pub fn with_expiry_threshold(device_mac: [u8; 6], expiry_threshold_ms: u64) -> Self {
        Self {
            sync_expiry_threshold_ms: expiry_threshold_ms,
            ..Self::new(device_mac)
        }
    }

    /// Handle time broadcast message
    pub fn handle_time_broadcast(&mut self, message: &Message) -> Result<()> {
        self.stats.sync_count = self.stats.sync_count.saturating_add(1);

        // Verify message source and target (simplified validation)
        match (&message.header.source, &message.header.target) {
            (NodeId::Edge(_), NodeId::Any) => {} // Broadcast from edge - OK
            (NodeId::Edge(_), NodeId::Device(mac)) if *mac == self.device_mac => {} // Direct message - OK
            _ => return Err(Error::InvalidCommand), // Invalid source/target
        }

        if let MessagePayload::TimeSync(TimeSyncPayload::Broadcast {
            timestamp,
            accuracy_ms,
            ..
        }) = &message.payload
        {
            let current_uptime = self.time_provider.monotonic_time_ms();
            let timestamp_ms =
                timestamp.unix_timestamp() as u64 * 1000 + timestamp.millisecond() as u64;

            // Update time synchronization
            self.time_offset_ms = timestamp_ms as i64 - current_uptime as i64;
            self.last_sync_time = Some(current_uptime);
            self.sync_state = DeviceSyncState::Synced;

            // Update stats
            self.stats.success_count = self.stats.success_count.saturating_add(1);
            self.stats.last_accuracy_ms = *accuracy_ms;

            Ok(())
        } else {
            Err(Error::InvalidCommand)
        }
    }

    /// Handle time sync response message
    pub fn handle_time_sync_response(&mut self, message: &Message) -> Result<()> {
        self.stats.sync_count = self.stats.sync_count.saturating_add(1);

        // Verify message source and target
        match (&message.header.source, &message.header.target) {
            (NodeId::Edge(_), NodeId::Device(mac)) if *mac == self.device_mac => {} // Response from edge - OK
            _ => return Err(Error::InvalidCommand), // Invalid source/target
        }

        if let MessagePayload::TimeSync(TimeSyncPayload::Response {
            request_receive_time: _,
            response_send_time,
            accuracy_ms,
            ..
        }) = &message.payload
        {
            let current_uptime = self.time_provider.monotonic_time_ms();

            // Simple offset calculation using response send time
            let response_timestamp_ms = response_send_time.unix_timestamp() as u64 * 1000
                + response_send_time.millisecond() as u64;

            // Update time synchronization
            self.time_offset_ms = response_timestamp_ms as i64 - current_uptime as i64;
            self.last_sync_time = Some(current_uptime);
            self.sync_state = DeviceSyncState::Synced;

            // Update stats
            self.stats.success_count = self.stats.success_count.saturating_add(1);
            self.stats.last_accuracy_ms = *accuracy_ms;

            Ok(())
        } else {
            Err(Error::InvalidCommand)
        }
    }

    /// Set time offset directly (for testing)
    pub fn set_time_offset(&mut self, offset_ms: i64) {
        self.time_offset_ms = offset_ms;
        self.last_sync_time = Some(self.time_provider.monotonic_time_ms());
        self.sync_state = DeviceSyncState::Synced;
    }

    /// Update synchronization state based on expiry
    pub fn update_sync_state(&mut self) {
        if let Some(last_sync) = self.last_sync_time {
            let current_time = self.time_provider.monotonic_time_ms();
            if current_time.saturating_sub(last_sync) > self.sync_expiry_threshold_ms {
                self.sync_state = DeviceSyncState::Expired;
            }
        }
    }

    /// Check if device is synchronized
    pub fn is_synced(&self) -> bool {
        match self.sync_state {
            DeviceSyncState::Synced => {
                if let Some(last_sync) = self.last_sync_time {
                    let current_time = self.time_provider.monotonic_time_ms();
                    current_time.saturating_sub(last_sync) <= self.sync_expiry_threshold_ms
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Get current synchronized time
    pub fn get_current_time(&self) -> OffsetDateTime {
        let current_uptime = self.time_provider.monotonic_time_ms();
        let adjusted_time = (current_uptime as i64 + self.time_offset_ms) as u64;

        OffsetDateTime::from_unix_timestamp_nanos((adjusted_time as i128) * 1_000_000)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    /// Get relative timestamp (milliseconds since device boot)
    pub fn get_relative_timestamp(&self) -> u64 {
        self.time_provider.monotonic_time_ms()
    }

    /// Get current time offset in milliseconds
    pub fn get_time_offset_ms(&self) -> i64 {
        self.time_offset_ms
    }

    /// Get sync success rate (0.0 to 1.0)
    pub fn get_sync_quality(&self) -> f32 {
        if self.stats.sync_count > 0 {
            self.stats.success_count as f32 / self.stats.sync_count as f32
        } else {
            0.0
        }
    }

    /// Get time since last sync in milliseconds
    pub fn time_since_last_sync_ms(&self) -> Option<u64> {
        self.last_sync_time.map(|last_sync| {
            let current_time = self.time_provider.monotonic_time_ms();
            current_time.saturating_sub(last_sync)
        })
    }

    /// Reset synchronization state
    pub fn reset(&mut self) {
        self.time_offset_ms = 0;
        self.last_sync_time = None;
        self.sync_state = DeviceSyncState::Unsynced;
        // Keep stats for monitoring
    }

    /// Reset statistics (for testing or periodic cleanup)
    pub fn reset_stats(&mut self) {
        self.stats = DeviceSyncStats::default();
    }
}
