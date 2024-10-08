mod auth;
mod control;
mod device;
mod group;
mod message;
mod region;

pub use auth::*;
pub use control::*;
pub use device::*;
pub use group::*;
pub use message::*;
pub use region::*;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub type Id = i32;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    /// Temperature reading in Celsius
    pub temperature: f32,
    /// Light intensity in lux
    pub illuminance: i32,
    /// Relative humidity percentage
    pub humidity: f32,
    /// Data collection timestamp
    pub timestamp: OffsetDateTime,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowData {
    /// Window position percentage (0-100)
    pub target_position: u8,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionSettingData {
    /// Target light intensity range in lux
    pub light_range: (i32, i32),
    /// Target temperature range in Celsius
    pub temperature_range: (f32, f32),
    /// Time period for these settings
    pub time_range: (OffsetDateTime, OffsetDateTime),
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSettingData {
    /// Target position range in percentage
    pub window_position: (u8, u8),
    /// Time period for these settings
    pub time_range: (OffsetDateTime, OffsetDateTime),
}
