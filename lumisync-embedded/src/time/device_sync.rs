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
        // Verify message source is from Edge node for security
        if !matches!(message.header.source, NodeId::Edge(_)) {
            return Err(Error::InvalidCommand);
        }

        // Verify target is broadcast or this specific device
        match message.header.target {
            NodeId::Any => {}                                   // Broadcast is OK
            NodeId::Device(mac) if mac == self.device_mac => {} // Direct message is OK
            _ => return Err(Error::InvalidCommand),             // Wrong target
        }

        if let MessagePayload::TimeSync(TimeSyncPayload::Broadcast { timestamp, .. }) =
            &message.payload
        {
            let current_uptime = self.time_provider.monotonic_time_ms();
            let timestamp_ms =
                timestamp.unix_timestamp() as u64 * 1000 + timestamp.millisecond() as u64;

            // Calculate device offset to match edge's synchronized time
            let new_offset = timestamp_ms as i64 - current_uptime as i64;

            self.time_offset_ms = new_offset;
            self.last_sync_time = Some(current_uptime);
            self.sync_state = DeviceSyncState::Synced;

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

    /// Reset synchronization state
    pub fn reset(&mut self) {
        self.time_offset_ms = 0;
        self.last_sync_time = None;
        self.sync_state = DeviceSyncState::Unsynced;
    }
}
