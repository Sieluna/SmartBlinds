use core::hash::Hash;

use serde::{Deserialize, Serialize};

pub type MacAddress = [u8; 6];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    EdgeController,
    Window,
    Sensor,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, Hash, PartialEq, Copy)]
pub struct DeviceId(pub u32);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SensorData {
    pub light: i32,
    pub temperature: i16,
    pub timestamp: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WindowState {
    pub position: i8,
    pub battery: u8,
}
