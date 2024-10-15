mod controller;
mod light_sensor;
mod temp_sensor;

pub use controller::*;
pub use light_sensor::LightSensor;
pub use temp_sensor::TempSensor;

use alloc::string::String;

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct SensorConfig {
    pub serial_port: &'static str,
    pub serial_baud_rate: u32,
    pub sensor_id: [u8; 24],
    pub publish_interval_ms: u32,
}

impl Default for SensorConfig {
    fn default() -> Self {
        let mut sensor_id = [0; 24];
        let prefix = b"SENSOR-";
        sensor_id[..7].copy_from_slice(prefix);

        Self {
            serial_port: "/dev/ttyUSB0",
            serial_baud_rate: 115200,
            sensor_id,
            publish_interval_ms: 1000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SensorData {
    /// Sensor ID
    pub sensor_id: String,
    /// Light value (lux)
    pub light: f32,
    /// Temperature value (℃)
    pub temperature: f32,
}

impl SensorData {
    pub fn new(sensor_id: String, light: f32, temperature: f32) -> Self {
        Self {
            sensor_id,
            light,
            temperature,
        }
    }

    pub fn to_json(&self) -> Result<String, Error> {
        let json = alloc::format!(
            "{{\"id\":\"{}\",\"lght\":{:.1},\"temp\":{:.1}}}",
            self.sensor_id,
            self.light,
            self.temperature
        );

        Ok(json)
    }
}
