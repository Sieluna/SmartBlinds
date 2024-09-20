use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSensorRequest {
    /// The region this sensor belongs to.
    pub region: i32,
    /// The name of sensor.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRangeQuery {
    pub start: Option<OffsetDateTime>,
    pub end: Option<OffsetDateTime>,
}
