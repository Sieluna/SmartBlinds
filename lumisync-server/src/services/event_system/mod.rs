mod event_bus;
mod event_storage;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use event_bus::*;
pub use event_storage::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventPayload {
    SensorData {
        sensor_id: i32,
        light: i32,
        temperature: f32,
        timestamp: OffsetDateTime,
    },
    WeatherData {
        location: String,
        temperature: f32,
        humidity: f32,
        wind_speed: f32,
        solar_radiation: f32,
        timestamp: OffsetDateTime,
    },
    RegionData {
        region_id: i32,
        indoor_temperature: f32,
        indoor_light: i32,
        outdoor_temperature: f32,
        timestamp: OffsetDateTime,
    },

    DeviceStatus {
        device_id: String,
        device_type: String,
        status: String,
        timestamp: OffsetDateTime,
    },

    GuideCommand {
        region_id: i32,
        state: String,
        confidence: f32,
        timestamp: OffsetDateTime,
    },
    UserCommand {
        user_id: i32,
        command: String,
        timestamp: OffsetDateTime,
    },

    CommandResult {
        command_id: String,
        device_id: String,
        success: bool,
        message: String,
        timestamp: OffsetDateTime,
    },

    Generic {
        event_type: String,
        data: String,
        timestamp: OffsetDateTime,
    },
}
