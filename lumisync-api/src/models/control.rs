use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{Id, WindowData};

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Set window position
    SetWindow {
        /// Target device identifier
        device_id: Id,
        /// Window position data
        #[serde(flatten)]
        data: WindowData,
    },
    /// Start device calibration
    Calibrate {
        /// Target device identifier
        device_id: Id,
    },
    /// Stop all operations
    EmergencyStop {
        /// Target region identifier
        device_id: Id,
    },
    /// Request device status update
    RequestStatus {
        /// Target device identifier
        device_id: Id,
    },
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    /// Operation result message
    pub message: String,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCommandRequest {
    /// Command list
    pub commands: Vec<Command>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCommand {
    /// Scene identifier
    pub scene_id: Id,
    /// Scene name
    pub name: String,
    /// Scene description
    pub description: Option<String>,
    /// Associated commands
    pub commands: Vec<Command>,
}
