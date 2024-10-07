mod application;
mod device;
mod error;

pub use application::*;
pub use device::*;
pub use error::ErrorCode;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Normal operation message.
    Regular,
    /// High priority message that requires immediate attention.
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    /// Temperature reading in Celsius.
    pub temperature: f32,
    /// Light intensity in lux.
    pub illuminance: i32,
    /// Relative humidity percentage.
    pub humidity: f32,
    /// Data collection timestamp.
    pub timestamp: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowData {
    /// Window position percentage (0-100).
    pub target_position: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionSettingData {
    /// Target light intensity range in lux.
    pub light_range: (i32, i32),
    /// Target temperature range in Celsius.
    pub temperature_range: (f32, f32),
    /// Time period for these settings.
    pub time_range: (OffsetDateTime, OffsetDateTime),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSettingData {
    /// Target position range in percentage.
    pub window_position: (u8, u8),
    /// Time period for these settings.
    pub time_range: (OffsetDateTime, OffsetDateTime),
}
