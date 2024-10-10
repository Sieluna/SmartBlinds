use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::control::Command;
use super::device::DeviceStatus;
use super::settings::{RegionSettingData, WindowSettingData};
use super::{Id, WindowData};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Normal operation message
    Regular,
    /// High priority message that requires immediate attention
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message metadata
    pub header: MessageHeader,
    /// Message content
    pub payload: MessagePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    /// Unique message identifier
    pub id: Uuid,
    /// Message creation timestamp
    pub timestamp: OffsetDateTime,
    /// Message priority level
    pub priority: Priority,
    /// Message source identifier
    pub source: String,
    /// Message destination identifier
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Command from cloud to edge devices
    CloudCommand(CloudCommand),
    /// Status report from edge to cloud
    EdgeReport(EdgeReport),
    /// Success confirmation response
    Acknowledge(AckPayload),
    /// Error response with details
    Error(ErrorPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudCommand {
    /// Update region environmental settings
    ConfigureRegion {
        /// Target region identifier
        region_id: Id,
        /// New region settings
        plan: Vec<RegionSettingData>,
    },
    /// Update window control settings
    ConfigureWindow {
        /// Target window identifier
        window_id: Id,
        /// New window settings
        plan: Vec<WindowSettingData>,
    },
    /// Send commands to multiple devices
    ControlDevices {
        /// Target region identifier
        region_id: Id,
        /// Device commands map
        commands: BTreeMap<Id, Command>,
    },
    /// Send optimization suggestions
    SendAnalyse {
        /// Target region identifier
        region_id: Id,
        /// Suggested window positions
        windows: Vec<WindowData>,
        /// Analysis explanation
        reason: String,
        /// Confidence score
        confidence: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeReport {
    /// Device status update
    DeviceStatus {
        /// Source region identifier
        region_id: Id,
        /// Device status list
        devices: Vec<DeviceStatus>,
    },
    /// Edge system health metrics
    HealthReport {
        /// CPU usage percentage
        cpu_usage: f32,
        /// Memory usage percentage
        memory_usage: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckPayload {
    /// Reference to original message
    pub original_msg_id: Uuid,
    /// Operation status
    pub status: String,
    /// Additional status information
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Reference to failed message
    pub original_msg_id: Option<Uuid>,
    /// Error type
    pub code: ErrorCode,
    /// Error description
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    /// Invalid input parameters
    InvalidRequest,
    /// Device not connected
    DeviceOffline,
    /// Operation not allowed
    PermissionDenied,
    /// Resource limit exceeded
    OverLimit,
    /// Internal processing error
    InternalError,
    /// Device hardware error
    HardwareFailure,
    /// Communication error
    NetworkError,
    /// Critical battery level
    BatteryLow,
    /// Operation time exceeded
    Timeout,
}
