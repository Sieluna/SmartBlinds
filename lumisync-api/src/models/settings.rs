use alloc::collections::BTreeMap;
use alloc::string::String;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::Id;

/// Generic setting interface
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting<T> {
    /// Setting ID
    pub id: Option<Id>,
    /// Target object ID (region ID or device ID)
    pub target_id: Id,
    /// Setting data
    pub data: T,
    /// Effective start time
    pub start_time: OffsetDateTime,
    /// Effective end time
    pub end_time: OffsetDateTime,
}

/// Region environment setting data
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionSettingData {
    /// Target light intensity range (lux)
    pub light_range: (i32, i32),
    /// Target temperature range (Celsius)
    pub temperature_range: (f32, f32),
    /// Target humidity range (percentage)
    pub humidity_range: (f32, f32),
}

/// Window control setting data
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSettingData {
    /// Target position range (percentage)
    pub position_range: (u8, u8),
    /// Auto-adjust mode
    pub auto_adjust: bool,
}

/// Sensor setting data
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorSettingData {
    /// Sample interval (seconds)
    pub sample_interval: u32,
    /// Report thresholds
    pub report_threshold: BTreeMap<String, f32>,
}

/// Setting creation request
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSettingRequest<T> {
    /// Target object ID
    pub target_id: Id,
    /// Setting data
    pub data: T,
    /// Start time
    pub start_time: OffsetDateTime,
    /// End time
    pub end_time: OffsetDateTime,
}

/// Setting update request
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingRequest<T> {
    /// Setting data
    pub data: Option<T>,
    /// Start time
    pub start_time: Option<OffsetDateTime>,
    /// End time
    pub end_time: Option<OffsetDateTime>,
}

/// Generic setting response
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingResponse<T> {
    /// Setting ID
    pub id: Id,
    /// Target object ID
    pub target_id: Id,
    /// Setting data
    pub data: T,
    /// Start time
    pub start_time: OffsetDateTime,
    /// End time
    pub end_time: OffsetDateTime,
}
