mod application;
mod device;
mod error;

pub use application::{AppHeader, AppMessage, AppPayload, CloudCommand, EdgeReport};
pub use device::{DeviceCommand, DeviceFrame, DeviceStatus};
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
    /// timestamp of the sensor data
    pub timestamp: OffsetDateTime,
}

/// Window Data Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowData {
    /// window position (0-100%)
    pub target_position: u8,
}

/// Region Policy Data Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyData {
    /// Target light range
    pub light_range: (i32, i32),
    /// Target temperature range
    pub temperature_range: (f32, f32),
}
