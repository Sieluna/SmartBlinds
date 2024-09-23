use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::device::{DeviceCommand, DeviceStatus};
use super::error::ErrorCode;
use super::{Priority, RegionSettingData, WindowData, WindowSettingData};

/// Application Message Container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMessage {
    /// Message metadata
    pub header: AppHeader,
    /// Message content
    pub payload: AppPayload,
}

/// Application Message Header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppHeader {
    /// Message unique identifier
    pub id: Uuid,
    /// Message timestamp
    pub timestamp: OffsetDateTime,
    /// Message priority
    pub priority: Priority,
    /// Source identifier
    pub source: String,
    /// Destination identifier
    pub destination: String,
}

/// Application Payload Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppPayload {
    /// Cloud to Edge command
    CloudCommand(CloudCommand),
    /// Edge to Cloud report
    EdgeReport(EdgeReport),
    /// Acknowledgment response
    Acknowledge(AckPayload),
    /// Error response
    Error(ErrorPayload),
}

/// Cloud Command Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudCommand {
    /// Configure a region's settings
    ConfigureRegion {
        /// Target region ID
        region_id: i32,
        /// Region plan settings
        plan: Vec<RegionSettingData>,
    },
    /// Configure a window's settings
    ConfigureWindow {
        /// Target window ID
        window_id: i32,
        /// Window plan settings
        plan: Vec<WindowSettingData>,
    },
    /// Control multiple devices
    ControlDevices {
        /// Target region ID
        region_id: i32,
        /// Target device IDs and commands
        commands: HashMap<i32, DeviceCommand>,
    },
    /// Send cloud analysis recommendations
    SendAnalyse {
        /// Target region ID
        region_id: i32,
        /// Recommended window positions
        windows: Vec<WindowData>,
        /// Explanation for the analysis
        reason: String,
        /// Analysis confidence level (0.0-1.0)
        confidence: f32,
    },
}

/// Edge Report Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeReport {
    /// Device status report
    DeviceStatus {
        /// Target region ID
        region_id: i32,
        /// Device statuses
        devices: Vec<DeviceStatus>,
    },
    /// System health report
    HealthReport {
        /// CPU usage percentage
        cpu_usage: f32,
        /// Memory usage percentage
        memory_usage: f32,
    },
}

/// Acknowledgment Payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckPayload {
    /// Original message ID
    pub original_msg_id: Uuid,
    /// Status information
    pub status: String,
    /// Status details (optional)
    pub details: Option<String>,
}

/// Error Payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Original message ID (if applicable)
    pub original_msg_id: Option<Uuid>,
    /// Error code
    pub code: ErrorCode,
    /// Error message
    pub message: String,
}
