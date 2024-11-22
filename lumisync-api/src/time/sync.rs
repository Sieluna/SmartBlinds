use alloc::vec::Vec;
use core::time::Duration;

use super::SyncError;
use crate::message::NodeId;

/// Time offset information
#[derive(Debug, Clone)]
pub struct TimeOffset {
    /// Local timestamp (based on Instant)
    pub local_instant: u64,
    /// Reference timestamp (from parent node)
    pub reference_time: u64,
    /// Network transmission delay
    pub network_delay: Duration,
    /// Confidence level at measurement time (0.0-1.0)
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    /// Not synchronized
    Unsynced,
    /// Currently synchronizing
    Syncing,
    /// Successfully synchronized
    Synced,
    /// Synchronization failed
    Failed,
}

#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Sync interval (milliseconds)
    pub sync_interval_ms: u64,
    /// Maximum allowed time drift (milliseconds)
    pub max_drift_ms: u64,
    /// Time offset history record count
    pub offset_history_size: usize,
    /// Network delay compensation threshold
    pub delay_compensation_threshold_ms: u64,
    /// Sync retry count
    pub max_retry_count: u8,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            sync_interval_ms: 30000, // 30 seconds
            max_drift_ms: 1000,      // 1 second
            offset_history_size: 10,
            delay_compensation_threshold_ms: 100,
            max_retry_count: 3,
        }
    }
}

pub struct TimeSynchronizer {
    node_id: NodeId, // Use NodeId instead of NodeType
    config: SyncConfig,
    status: SyncStatus,
    offset_history: Vec<TimeOffset>,
    last_sync_time: Option<u64>,
    current_offset_ms: i64,
    retry_count: u8,
}

impl TimeSynchronizer {
    pub fn new(node_id: NodeId, config: SyncConfig) -> Self {
        let capacity = config.offset_history_size;
        Self {
            node_id,
            config,
            status: SyncStatus::Unsynced,
            offset_history: Vec::with_capacity(capacity),
            last_sync_time: None,
            current_offset_ms: 0,
            retry_count: 0,
        }
    }

    /// Process received time synchronization message
    pub fn handle_sync_request(
        &mut self,
        local_timestamp: u64,
        network_delay: Duration,
    ) -> TimeOffset {
        let offset = TimeOffset {
            local_instant: local_timestamp,
            reference_time: self.get_adjusted_time(local_timestamp),
            network_delay,
            confidence: self.calculate_confidence(&network_delay),
        };

        self.add_offset_sample(offset.clone());
        offset
    }

    /// Process received time synchronization response
    pub fn handle_sync_response(
        &mut self,
        request_time: u64,
        response_time: u64,
        local_receive_time: u64,
    ) -> Result<(), SyncError> {
        let round_trip_time = local_receive_time.saturating_sub(request_time);
        let network_delay = Duration::from_millis(round_trip_time / 2);

        // Calculate time offset
        let estimated_response_time = request_time + (round_trip_time / 2);
        let offset_ms = response_time as i64 - estimated_response_time as i64;

        let offset = TimeOffset {
            local_instant: estimated_response_time,
            reference_time: response_time,
            network_delay,
            confidence: self.calculate_confidence(&network_delay),
        };

        self.add_offset_sample(offset);
        self.update_time_offset(offset_ms);

        self.status = SyncStatus::Synced;
        self.last_sync_time = Some(local_receive_time);
        self.retry_count = 0;

        Ok(())
    }

    /// Add time offset sample
    fn add_offset_sample(&mut self, offset: TimeOffset) {
        if self.offset_history.len() >= self.config.offset_history_size {
            self.offset_history.remove(0);
        }
        self.offset_history.push(offset);
    }

    /// Calculate confidence
    fn calculate_confidence(&self, network_delay: &Duration) -> f32 {
        let delay_ms = network_delay.as_millis() as u64;
        if delay_ms > self.config.delay_compensation_threshold_ms {
            // Network delay too high, confidence decreases
            (self.config.delay_compensation_threshold_ms as f32 / delay_ms as f32).min(1.0)
        } else {
            // Calculate confidence based on consistency of historical data
            if self.offset_history.len() < 2 {
                0.5
            } else {
                self.calculate_consistency_confidence()
            }
        }
    }

    /// Calculate confidence based on consistency of historical data
    fn calculate_consistency_confidence(&self) -> f32 {
        if self.offset_history.len() < 2 {
            return 0.5;
        }

        let recent_offsets: Vec<i64> = self
            .offset_history
            .iter()
            .rev()
            .take(5)
            .map(|offset| offset.reference_time as i64 - offset.local_instant as i64)
            .collect();

        if recent_offsets.len() < 2 {
            return 0.5;
        }

        // Calculate standard deviation
        let mean = recent_offsets.iter().sum::<i64>() as f32 / recent_offsets.len() as f32;
        let variance = recent_offsets
            .iter()
            .map(|&x| (x as f32 - mean).powi(2))
            .sum::<f32>()
            / recent_offsets.len() as f32;
        let std_dev = variance.sqrt();

        // Standard deviation smaller, confidence higher
        (100.0 / (std_dev + 1.0)).min(1.0)
    }

    /// Update time offset
    fn update_time_offset(&mut self, new_offset_ms: i64) {
        // Use weighted average to smooth time offset
        if self.offset_history.len() > 1 {
            let weight = 0.3; // New value weight
            self.current_offset_ms = (self.current_offset_ms as f64 * (1.0 - weight)
                + new_offset_ms as f64 * weight) as i64;
        } else {
            self.current_offset_ms = new_offset_ms;
        }
    }

    /// Get adjusted time
    pub fn get_adjusted_time(&self, local_time: u64) -> u64 {
        (local_time as i64 + self.current_offset_ms).max(0) as u64
    }

    /// Check if synchronization is needed
    pub fn needs_sync(&self, current_time: u64) -> bool {
        match self.last_sync_time {
            None => true,
            Some(last_sync) => {
                current_time.saturating_sub(last_sync) > self.config.sync_interval_ms
            }
        }
    }

    /// Get synchronization status
    pub fn get_status(&self) -> SyncStatus {
        self.status.clone()
    }

    /// Get current time offset
    pub fn get_current_offset_ms(&self) -> i64 {
        self.current_offset_ms
    }

    /// Reset synchronization status
    pub fn reset(&mut self) {
        self.status = SyncStatus::Unsynced;
        self.offset_history.clear();
        self.last_sync_time = None;
        self.current_offset_ms = 0;
        self.retry_count = 0;
    }

    /// Handle synchronization failure
    pub fn handle_sync_failure(&mut self) {
        self.retry_count += 1;
        if self.retry_count >= self.config.max_retry_count {
            self.status = SyncStatus::Failed;
        } else {
            self.status = SyncStatus::Unsynced;
        }
    }

    /// Get node-specific timing parameters based on NodeId
    fn get_timing_parameters(&self) -> (u64, u32, u16) {
        match self.node_id {
            NodeId::Cloud => (
                30000, // sync_interval_ms
                50,    // processing_delay_ms
                1,     // precision_ms
            ),
            NodeId::Edge(_) => (
                10000, // sync_interval_ms
                20,    // processing_delay_ms
                10,    // precision_ms
            ),
            NodeId::Device(_) => (
                5000, // sync_interval_ms
                10,   // processing_delay_ms
                50,   // precision_ms
            ),
        }
    }

    /// Check if this node can act as a time authority
    pub fn can_broadcast_time(&self) -> bool {
        matches!(self.node_id, NodeId::Edge(_))
    }

    /// Get sync hierarchy level (lower = higher authority)
    pub fn get_hierarchy_level(&self) -> u8 {
        match self.node_id {
            NodeId::Cloud => 0,
            NodeId::Edge(_) => 1,
            NodeId::Device(_) => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::NodeId;

    fn create_test_config() -> SyncConfig {
        SyncConfig {
            sync_interval_ms: 1000,
            max_drift_ms: 100,
            offset_history_size: 5,
            delay_compensation_threshold_ms: 50,
            max_retry_count: 3,
        }
    }

    #[test]
    fn test_synchronizer_creation() {
        let config = create_test_config();
        let sync = TimeSynchronizer::new(NodeId::Edge(1), config);

        assert_eq!(sync.get_status(), SyncStatus::Unsynced);
        assert_eq!(sync.get_current_offset_ms(), 0);
        assert!(sync.needs_sync(1000));
    }

    #[test]
    fn test_sync_response_handling() {
        let config = create_test_config();
        let mut sync = TimeSynchronizer::new(NodeId::Device([1, 2, 3, 4, 5, 6]), config);

        let request_time = 1000;
        let response_time = 1050;
        let receive_time = 1100;

        let result = sync.handle_sync_response(request_time, response_time, receive_time);
        assert!(result.is_ok());
        assert_eq!(sync.get_status(), SyncStatus::Synced);
    }

    #[test]
    fn test_time_adjustment() {
        let config = create_test_config();
        let mut sync = TimeSynchronizer::new(NodeId::Edge(1), config);

        // Simulate a sync that adds 100ms offset
        let _ = sync.handle_sync_response(1000, 1150, 1100);

        let local_time = 2000;
        let adjusted_time = sync.get_adjusted_time(local_time);

        // Should have some offset applied
        assert_ne!(adjusted_time, local_time);
    }

    #[test]
    fn test_sync_interval_checking() {
        let mut config = create_test_config();
        config.sync_interval_ms = 500;
        let mut sync = TimeSynchronizer::new(NodeId::Device([1, 2, 3, 4, 5, 6]), config);

        // Initially needs sync
        assert!(sync.needs_sync(1000));

        // After sync, shouldn't need sync immediately
        let _ = sync.handle_sync_response(1000, 1050, 1100);
        assert!(!sync.needs_sync(1200)); // Only 200ms later

        // After interval passes, should need sync again
        assert!(sync.needs_sync(1700)); // 700ms later
    }

    #[test]
    fn test_confidence_calculation() {
        let config = create_test_config();
        let sync = TimeSynchronizer::new(NodeId::Cloud, config);

        let low_delay = Duration::from_millis(10);
        let high_delay = Duration::from_millis(200);

        let low_confidence = sync.calculate_confidence(&high_delay);
        let high_confidence = sync.calculate_confidence(&low_delay);

        assert!(high_confidence >= low_confidence);
    }

    #[test]
    fn test_offset_history() {
        let mut config = create_test_config();
        config.offset_history_size = 3;
        let mut sync = TimeSynchronizer::new(NodeId::Edge(1), config);

        // Add multiple offsets
        for i in 0..5 {
            let offset = TimeOffset {
                local_instant: 1000 + i * 100,
                reference_time: 1050 + i * 100,
                network_delay: Duration::from_millis(25),
                confidence: 0.8,
            };
            sync.add_offset_sample(offset);
        }

        // Should only keep the latest 3
        assert_eq!(sync.offset_history.len(), 3);
    }

    #[test]
    fn test_sync_failure_handling() {
        let config = create_test_config();
        let mut sync = TimeSynchronizer::new(NodeId::Device([1, 2, 3, 4, 5, 6]), config);

        // Initial state
        assert_eq!(sync.get_status(), SyncStatus::Unsynced);

        // Fail a few times
        sync.handle_sync_failure();
        assert_eq!(sync.get_status(), SyncStatus::Unsynced);

        sync.handle_sync_failure();
        sync.handle_sync_failure();
        assert_eq!(sync.get_status(), SyncStatus::Failed);
    }

    #[test]
    fn test_reset_functionality() {
        let config = create_test_config();
        let mut sync = TimeSynchronizer::new(NodeId::Edge(1), config);

        // Set up some state
        let _ = sync.handle_sync_response(1000, 1050, 1100);
        sync.handle_sync_failure();

        assert_eq!(sync.get_status(), SyncStatus::Unsynced);
        assert!(!sync.offset_history.is_empty());

        // Reset
        sync.reset();

        assert_eq!(sync.get_status(), SyncStatus::Unsynced);
        assert!(sync.offset_history.is_empty());
        assert_eq!(sync.get_current_offset_ms(), 0);
    }

    #[test]
    fn test_consistency_confidence() {
        let config = create_test_config();
        let mut sync = TimeSynchronizer::new(NodeId::Cloud, config);

        // Add consistent offsets
        for i in 0..3 {
            let offset = TimeOffset {
                local_instant: 1000 + i * 100,
                reference_time: 1050 + i * 100, // Consistent 50ms offset
                network_delay: Duration::from_millis(20),
                confidence: 0.5,
            };
            sync.add_offset_sample(offset);
        }

        let confidence = sync.calculate_consistency_confidence();
        assert!(confidence > 0.0);
        assert!(confidence <= 1.0);
    }

    #[test]
    fn test_hierarchy_levels() {
        let config = create_test_config();

        let cloud = TimeSynchronizer::new(NodeId::Cloud, config.clone());
        let edge = TimeSynchronizer::new(NodeId::Edge(1), config.clone());
        let device = TimeSynchronizer::new(NodeId::Device([1, 2, 3, 4, 5, 6]), config);

        assert_eq!(cloud.get_hierarchy_level(), 0);
        assert_eq!(edge.get_hierarchy_level(), 1);
        assert_eq!(device.get_hierarchy_level(), 2);

        assert!(!cloud.can_broadcast_time());
        assert!(edge.can_broadcast_time());
        assert!(!device.can_broadcast_time());
    }
}
