mod pid;
mod system;

pub use pid::*;
pub use system::*;

use core::time::Duration;

use crate::types::*;

#[derive(Debug, Clone)]
pub struct PIDParams {
    pub kp: f32,
    pub ki: f32,
    pub kd: f32,
    pub min_output: f32,
    pub max_output: f32,
}

impl Default for PIDParams {
    fn default() -> Self {
        Self {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            min_output: -100.0,
            max_output: 100.0,
        }
    }
}

#[derive(Debug)]
pub struct ZoneStrategy {
    pub zone_id: u8,
    pub target_light: i32,
    pub target_temperature: i16,
    pub update_interval: Duration,
    pub pid_params: PIDParams,
}

#[derive(Debug)]
pub struct DeviceControlState {
    pub device_id: DeviceId,
    pub last_command: Option<ControlCommand>,
    pub last_update: u32,
    pub error_count: u8,
}

#[derive(Debug)]
pub struct ControlConfig {
    pub default_update_interval: Duration,
    pub command_timeout: Duration,
    pub max_retries: u8,
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            default_update_interval: Duration::from_secs(5),
            command_timeout: Duration::from_secs(1),
            max_retries: 3,
        }
    }
}
