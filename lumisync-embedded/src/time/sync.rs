use embassy_time::{Duration, Instant};
use time::OffsetDateTime;

pub struct TimeSync {
    /// Offset from boot time to UTC (milliseconds)
    utc_offset_ms: i64,
    /// Device boot time
    boot_instant: Instant,
    /// Last sync time
    last_sync: Option<Instant>,
    /// Sync interval (default: 1 hour)
    sync_interval: Duration,
}

impl TimeSync {
    pub fn new() -> Self {
        Self {
            utc_offset_ms: 0,
            boot_instant: Instant::now(),
            last_sync: None,
            sync_interval: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Set sync interval
    pub fn set_sync_interval(&mut self, interval: Duration) {
        self.sync_interval = interval;
    }

    /// Sync with cloud time
    pub fn sync(&mut self, cloud_utc: OffsetDateTime) {
        let boot_elapsed_ms = Instant::now().duration_since(self.boot_instant).as_millis() as i64;
        let cloud_ms =
            cloud_utc.unix_timestamp() * 1000 + (cloud_utc.nanosecond() / 1_000_000) as i64;

        self.utc_offset_ms = cloud_ms - boot_elapsed_ms;
        self.last_sync = Some(Instant::now());

        log::info!("Time synced, offset: {} ms", self.utc_offset_ms);
    }

    /// Get current UTC time
    pub fn now_utc(&self) -> OffsetDateTime {
        let boot_elapsed_ms = Instant::now().duration_since(self.boot_instant).as_millis() as i64;
        let utc_ms = boot_elapsed_ms as i128 + self.utc_offset_ms as i128;

        OffsetDateTime::from_unix_timestamp_nanos(utc_ms * 1_000_000)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    /// Get milliseconds since boot
    pub fn uptime_ms(&self) -> u64 {
        Instant::now().duration_since(self.boot_instant).as_millis()
    }

    /// Check if sync is needed
    pub fn needs_sync(&self) -> bool {
        match self.last_sync {
            None => true,
            Some(last) => Instant::now().duration_since(last) >= self.sync_interval,
        }
    }

    /// Convert device uptime to UTC
    pub fn uptime_to_utc(&self, uptime_ms: u64) -> OffsetDateTime {
        let utc_ms = uptime_ms as i64 + self.utc_offset_ms;
        OffsetDateTime::from_unix_timestamp(utc_ms / 1000).unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    /// Check if recently synced
    pub fn is_synced(&self) -> bool {
        self.last_sync.is_some()
    }
}

impl Default for TimeSync {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_sync() {
        let mut sync = TimeSync::new();

        // Test sync with current time
        let now = OffsetDateTime::now_utc();
        sync.sync(now);

        let synced_time = sync.now_utc();
        let diff = (synced_time.unix_timestamp() - now.unix_timestamp()).abs();
        assert!(diff <= 1, "Time difference should be less than 1 second");

        assert!(
            !sync.needs_sync(),
            "Should not need sync immediately after syncing"
        );
        assert!(sync.is_synced(), "Should be marked as synced");
    }

    #[test]
    fn test_uptime_conversion() {
        let mut sync = TimeSync::new();
        let sync_time = OffsetDateTime::from_unix_timestamp(1609459200).unwrap(); // 2021-01-01
        sync.sync(sync_time);

        let uptime_ms = 5000; // 5 seconds
        let converted = sync.uptime_to_utc(uptime_ms);

        // The converted time should be approximately sync_time + 5 seconds
        let expected_timestamp = sync_time.unix_timestamp() + 5;
        let actual_timestamp = converted.unix_timestamp();

        assert!(
            (actual_timestamp - expected_timestamp).abs() <= 1,
            "Converted timestamp should be within 1 second of expected"
        );
    }

    #[test]
    fn test_sync_interval() {
        let mut sync = TimeSync::new();

        // Set custom sync interval
        sync.set_sync_interval(Duration::from_secs(1800)); // 30 minutes

        let now = OffsetDateTime::now_utc();
        sync.sync(now);

        assert!(!sync.needs_sync(), "Should not need sync immediately");
    }

    #[test]
    fn test_offset_calculation() {
        let mut sync = TimeSync::new();

        // Test with a known time
        let known_time = OffsetDateTime::from_unix_timestamp(1609459200).unwrap(); // 2021-01-01
        sync.sync(known_time);

        // Get uptime and verify offset calculation
        let uptime = sync.uptime_ms();
        let current_utc = sync.now_utc();

        // The difference should be approximately the known time
        let expected_diff = known_time.unix_timestamp() * 1000 + uptime as i64;
        let actual_diff = current_utc.unix_timestamp() * 1000;

        assert!(
            (actual_diff - expected_diff).abs() <= 1000, // Within 1 second
            "Time calculation should be consistent"
        );
    }

    #[test]
    fn test_multiple_syncs() {
        let mut sync = TimeSync::new();

        // First sync
        let time1 = OffsetDateTime::from_unix_timestamp(1609459200).unwrap();
        sync.sync(time1);
        assert!(sync.is_synced());

        // Second sync after some time
        let time2 = OffsetDateTime::from_unix_timestamp(1609459260).unwrap(); // 1 minute later
        sync.sync(time2);
        assert!(sync.is_synced());

        // Time should be consistent
        let current = sync.now_utc();
        assert!(current.unix_timestamp() >= time2.unix_timestamp());
    }
}
