use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::device::{DeviceCommand, DeviceStatus};
use super::error::ErrorCode;
use super::{Priority, RegionSettingData, WindowData, WindowSettingData};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMessage {
    /// Message metadata.
    pub header: AppHeader,
    /// Message content.
    pub payload: AppPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppHeader {
    /// Unique message identifier.
    pub id: Uuid,
    /// Message creation timestamp.
    pub timestamp: OffsetDateTime,
    /// Message priority level.
    pub priority: Priority,
    /// Message source identifier.
    pub source: String,
    /// Message destination identifier.
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppPayload {
    /// Command from cloud to edge devices.
    CloudCommand(CloudCommand),
    /// Status report from edge to cloud.
    EdgeReport(EdgeReport),
    /// Success confirmation response.
    Acknowledge(AckPayload),
    /// Error response with details.
    Error(ErrorPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudCommand {
    /// Update region environmental settings.
    ConfigureRegion {
        /// Target region identifier.
        region_id: i32,
        /// New region settings.
        plan: Vec<RegionSettingData>,
    },
    /// Update window control settings.
    ConfigureWindow {
        /// Target window identifier.
        window_id: i32,
        /// New window settings.
        plan: Vec<WindowSettingData>,
    },
    /// Send commands to multiple devices.
    ControlDevices {
        /// Target region identifier.
        region_id: i32,
        /// Device commands map.
        commands: BTreeMap<i32, DeviceCommand>,
    },
    /// Send optimization suggestions.
    SendAnalyse {
        /// Target region identifier.
        region_id: i32,
        /// Suggested window positions.
        windows: Vec<WindowData>,
        /// Analysis explanation.
        reason: String,
        /// Confidence score.
        confidence: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeReport {
    /// Device status update.
    DeviceStatus {
        /// Source region identifier.
        region_id: i32,
        /// Device status list.
        devices: Vec<DeviceStatus>,
    },
    /// Edge system health metrics.
    HealthReport {
        /// CPU usage percentage.
        cpu_usage: f32,
        /// Memory usage percentage.
        memory_usage: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckPayload {
    /// Reference to original message.
    pub original_msg_id: Uuid,
    /// Operation status.
    pub status: String,
    /// Additional status information.
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Reference to failed message.
    pub original_msg_id: Option<Uuid>,
    /// Error type.
    pub code: ErrorCode,
    /// Error description.
    pub message: String,
}
