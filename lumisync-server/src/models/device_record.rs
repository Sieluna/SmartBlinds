use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use super::Table;

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeviceRecord {
    pub id: i32,
    pub device_id: i32,
    pub data: Value,
    /// The time of the record
    pub time: OffsetDateTime,
}

#[derive(Clone)]
pub struct DeviceRecordTable;

impl Table for DeviceRecordTable {
    fn name(&self) -> &'static str {
        "device_records"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS device_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                device_id INTEGER NOT NULL,
                data JSON NOT NULL,
                time TIMESTAMP NOT NULL,
                FOREIGN KEY (device_id) REFERENCES devices (id) ON DELETE CASCADE
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS device_records;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["devices"]
    }
}
