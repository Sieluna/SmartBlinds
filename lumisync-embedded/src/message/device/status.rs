use lumisync_api::Id;

#[derive(Debug, Clone)]
pub struct DeviceStatus {
    pub device_id: Id,
    pub current_position: u8,
    pub target_position: u8,
    pub battery_level: u8,
    pub is_moving: bool,
    pub error_code: u8,
}

impl Default for DeviceStatus {
    fn default() -> Self {
        Self {
            device_id: 0,
            current_position: 0,
            target_position: 0,
            battery_level: 100,
            is_moving: false,
            error_code: 0,
        }
    }
}
