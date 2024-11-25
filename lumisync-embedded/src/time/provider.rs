use embassy_time::Instant;
use lumisync_api::time::TimeProvider;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct EmbeddedTimeProvider {
    boot_instant: Instant,
    precision_ms: u16, // Timing precision in milliseconds
}

impl EmbeddedTimeProvider {
    pub fn new() -> Self {
        Self {
            boot_instant: Instant::now(),
            precision_ms: 50, // Default 50ms precision for embedded systems
        }
    }

    /// Create with custom precision
    pub fn with_precision(precision_ms: u16) -> Self {
        Self {
            boot_instant: Instant::now(),
            precision_ms,
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.boot_instant.elapsed().as_millis() as u64
    }

    /// Get timing precision in milliseconds
    pub fn precision_ms(&self) -> u16 {
        self.precision_ms
    }

    /// Set timing precision (for calibration)
    pub fn set_precision(&mut self, precision_ms: u16) {
        self.precision_ms = precision_ms;
    }
}

impl TimeProvider for EmbeddedTimeProvider {
    fn monotonic_time_ms(&self) -> u64 {
        self.elapsed_ms()
    }

    fn wall_clock_time(&self) -> Option<OffsetDateTime> {
        None
    }

    fn has_authoritative_time(&self) -> bool {
        false
    }
}

impl Default for EmbeddedTimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_monotonic() {
        let provider = EmbeddedTimeProvider::new();
        let time1 = provider.monotonic_time_ms();

        for _ in 0..1000 {
            core::hint::spin_loop();
        }

        let time2 = provider.monotonic_time_ms();

        assert!(time2 >= time1);
    }

    #[test]
    fn test_custom_precision() {
        let provider = EmbeddedTimeProvider::with_precision(25);
        assert_eq!(provider.precision_ms(), 25);
    }

    #[test]
    fn test_precision_update() {
        let mut provider = EmbeddedTimeProvider::new();
        assert_eq!(provider.precision_ms(), 50);

        provider.set_precision(100);
        assert_eq!(provider.precision_ms(), 100);
    }
}
