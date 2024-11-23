use lumisync_api::{Message, MessagePayload, TimeProvider, TimeSyncPayload};
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
    /// Time offset (milliseconds) - local time needs to add this offset to get synchronized time
    pub time_offset_ms: i64,
    /// Last sync time (local timestamp)
    pub last_sync_time: Option<u64>,
    /// Synchronization state
    pub sync_state: DeviceSyncState,
    /// Sync expiry threshold (milliseconds)
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
        if !matches!(message.header.source, lumisync_api::NodeId::Edge(_)) {
            return Err(Error::InvalidCommand);
        }

        // Verify target is broadcast or this specific device
        match message.header.target {
            lumisync_api::NodeId::Any => {} // Broadcast is OK
            lumisync_api::NodeId::Device(mac) if mac == self.device_mac => {} // Direct message is OK
            _ => return Err(Error::InvalidCommand),                           // Wrong target
        }

        if let MessagePayload::TimeSync(TimeSyncPayload::Broadcast {
            timestamp,
            offset_ms,
            accuracy_ms: _,
        }) = &message.payload
        {
            let current_uptime = self.time_provider.uptime_ms();

            // Convert timestamp to milliseconds since Unix epoch
            let timestamp_ms =
                timestamp.unix_timestamp() as u64 * 1000 + timestamp.millisecond() as u64;

            // The broadcast message contains:
            // - timestamp: Current time at the edge node
            // - offset_ms: Edge node's time offset relative to cloud/true time
            //
            // To synchronize, we need to calculate what offset to apply to our uptime
            // so that (uptime + our_offset) gives us the true time.
            //
            // True time = timestamp + offset_ms
            // We want: uptime + our_offset = true_time
            // Therefore: our_offset = true_time - uptime
            let true_time_ms = timestamp_ms as i64 + offset_ms;
            let new_offset = true_time_ms - current_uptime as i64;

            // Apply new offset
            self.time_offset_ms = new_offset;
            self.last_sync_time = Some(current_uptime);
            self.sync_state = DeviceSyncState::Synced;

            Ok(())
        } else {
            Err(Error::InvalidCommand)
        }
    }

    /// Directly set time offset (for testing)
    pub fn set_time_offset(&mut self, offset_ms: i64) {
        self.time_offset_ms = offset_ms;
        self.last_sync_time = Some(self.time_provider.uptime_ms());
        self.sync_state = DeviceSyncState::Synced;
    }

    /// Update synchronization state check
    pub fn update_sync_state(&mut self) {
        if let Some(last_sync) = self.last_sync_time {
            let current_time = self.time_provider.uptime_ms();
            if current_time.saturating_sub(last_sync) > self.sync_expiry_threshold_ms {
                self.sync_state = DeviceSyncState::Expired;
            }
        }
    }

    /// Whether synchronized (includes expiry check)
    pub fn is_synced(&self) -> bool {
        match self.sync_state {
            DeviceSyncState::Synced => {
                if let Some(last_sync) = self.last_sync_time {
                    let current_time = self.time_provider.uptime_ms();
                    current_time.saturating_sub(last_sync) <= self.sync_expiry_threshold_ms
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Get adjusted current time
    pub fn get_current_time(&self) -> OffsetDateTime {
        let current_uptime = self.time_provider.uptime_ms();
        let adjusted_time = (current_uptime as i64 + self.time_offset_ms) as u64;

        OffsetDateTime::from_unix_timestamp_nanos((adjusted_time as i128) * 1_000_000)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    /// Get relative timestamp (milliseconds since device boot)
    pub fn get_relative_timestamp(&self) -> u64 {
        self.time_provider.uptime_ms()
    }

    /// Reset synchronization state
    pub fn reset(&mut self) {
        self.time_offset_ms = 0;
        self.last_sync_time = None;
        self.sync_state = DeviceSyncState::Unsynced;
    }
}
