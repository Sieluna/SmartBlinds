use embassy_time::Instant;
use lumisync_api::TimeProvider;

#[derive(Debug, Clone)]
pub struct EmbeddedTimeProvider {
    boot_instant: Instant,
}

impl EmbeddedTimeProvider {
    pub fn new() -> Self {
        Self {
            boot_instant: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.boot_instant.elapsed().as_millis() as u64
    }
}

impl TimeProvider for EmbeddedTimeProvider {
    fn uptime_ms(&self) -> u64 {
        self.elapsed_ms()
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
        let time1 = provider.uptime_ms();

        for _ in 0..1000 {
            core::hint::spin_loop();
        }

        let time2 = provider.uptime_ms();

        assert!(time2 >= time1);
    }
}
