use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::Table;

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SensorRecord {
    pub id: i32,
    pub sensor_id: i32,
    /// Light in lux
    pub light: i32,
    /// Temperature in Celsius
    pub temperature: f32,
    /// Relative humidity %
    pub humidity: f32,
    /// The time of the record
    pub time: OffsetDateTime,
}

#[derive(Clone)]
pub struct SensorRecordTable;

impl Table for SensorRecordTable {
    fn name(&self) -> &'static str {
        "sensor_records"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS sensor_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sensor_id INTEGER NOT NULL,
                light INTEGER NOT NULL,
                temperature REAL NOT NULL,
                humidity REAL NOT NULL,
                time TIMESTAMP NOT NULL,
                FOREIGN KEY (sensor_id) REFERENCES sensors (id) ON DELETE CASCADE
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS sensor_records;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["sensors"]
    }
}
