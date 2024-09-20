use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSettingRequest {
    pub light: i32,
    pub temperature: f32,
    pub start: OffsetDateTime,
    pub end: OffsetDateTime,
    pub interval: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingResponse {
    pub light: Option<i32>,
    pub temperature: Option<f32>,
    pub start: Option<OffsetDateTime>,
    pub end: Option<OffsetDateTime>,
    pub interval: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingRequest {
    pub id: i32,
    pub light: i32,
    pub temperature: f32,
    pub start: OffsetDateTime,
    pub end: OffsetDateTime,
    pub interval: i32,
}
