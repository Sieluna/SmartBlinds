use embassy_time::{Duration, Timer};
use time::OffsetDateTime;

use crate::Result;

mod sync;
pub use sync::TimeSync;

pub struct TimeSyncService {
    time_sync: TimeSync,
}

impl TimeSyncService {
    pub fn new() -> Self {
        Self {
            time_sync: TimeSync::new(),
        }
    }

    /// Start auto sync task
    pub async fn start_sync_task<F, Fut>(&mut self, sync_fn: F) -> Result<()>
    where
        F: Fn() -> Fut + Copy,
        Fut: core::future::Future<Output = Result<OffsetDateTime>>,
    {
        loop {
            if self.time_sync.needs_sync() {
                match sync_fn().await {
                    Ok(cloud_time) => {
                        self.time_sync.sync(cloud_time);
                        log::info!("Auto sync successful");
                    }
                    Err(_) => {
                        log::warn!("Auto sync failed, will retry");
                    }
                }
            }
            Timer::after(Duration::from_secs(60)).await; // Check every minute
        }
    }

    pub fn get_time_sync(&self) -> &TimeSync {
        &self.time_sync
    }

    pub fn get_time_sync_mut(&mut self) -> &mut TimeSync {
        &mut self.time_sync
    }
}

impl Default for TimeSyncService {
    fn default() -> Self {
        Self::new()
    }
}
