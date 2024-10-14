mod auth;
mod control;
mod device;
mod group;
mod message;
mod region;
mod settings;

pub use auth::*;
pub use control::*;
pub use device::*;
pub use group::*;
pub use message::*;
pub use region::*;
pub use settings::*;

use alloc::vec::Vec;

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
pub struct Page<T> {
    /// Total number of items
    pub total: usize,
    /// Current page number
    pub page: usize,
    /// Total number of pages
    pub pages: usize,
    /// Items in the current page
    pub items: Vec<T>,
}
