use core::hash::Hash;

use heapless::Vec;
use serde::{Deserialize, Serialize};

pub type ShortString = heapless::String<32>;
pub type MacAddress = [u8; 6];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    EdgeController,
    SubController,
    Window,
    Sensor,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, Hash, PartialEq, Copy)]
pub struct DeviceId(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkZone {
    pub zone_id: u8,
    pub controller: DeviceId,
    pub devices: Vec<DeviceId, 16>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlCommand {
    SetPosition {
        window_id: DeviceId,
        position: i8,
    },
    RequestSensorData {
        sensor_id: DeviceId,
    },
    ConfigureDevice {
        device_id: DeviceId,
        config: DeviceConfig,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub sample_rate: u16,
    pub report_threshold: i16,
    pub power_mode: PowerMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PowerMode {
    Normal,
    LowPower,
    UltraLowPower,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    Advertisement(AdvertisementData),
    Command(ControlCommand),
    Response(ResponseData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvertisementData {
    pub device_id: DeviceId,
    pub node_type: NodeType,
    pub power_mode: PowerMode,
    pub rssi: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseData {
    SensorData(SensorData),
    WindowState(WindowState),
    Acknowledgement { command_id: u32 },
    Error { code: u8, message: ShortString },
}
