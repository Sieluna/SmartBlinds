mod application;
mod device;
mod error;

pub use application::*;
pub use device::*;
pub use error::ErrorCode;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Message Priority Levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Follow the normal lifecycle
    Regular,
    /// Emergency operations that override all others
    Emergency,
}

/// Sensor Data Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    /// Temperature reading in Celsius
    pub temperature: f32,
    /// Light intensity in lux
    pub illuminance: i32,
    /// Relative humidity percentage
    pub humidity: f32,
    /// Timestamp of the sensor data
    pub timestamp: OffsetDateTime,
}

/// Window Data Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowData {
    /// Window position (0-100%)
    pub target_position: u8,
}

/// Region Setting Data Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionSettingData {
    /// Target light range
    pub light_range: (i32, i32),
    /// Target temperature range
    pub temperature_range: (f32, f32),
    /// Target time range
    pub time_range: (OffsetDateTime, OffsetDateTime),
}

/// Window Setting Data Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSettingData {
    /// Target window position range
    pub window_position: (u8, u8),
    /// Target time range
    pub time_range: (OffsetDateTime, OffsetDateTime),
}
