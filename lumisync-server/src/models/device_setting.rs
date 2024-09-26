use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use super::Table;

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct DeviceSetting {
    pub id: i32,
    pub device_id: i32,
    pub setting: Value,
    pub start: OffsetDateTime,
    pub end: OffsetDateTime,
}

#[derive(Clone)]
pub struct DeviceSettingTable;

impl Table for DeviceSettingTable {
    fn name(&self) -> &'static str {
        "devices_settings"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS devices_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                device_id INTEGER NOT NULL,
                setting JSON NOT NULL,
                start TIMESTAMP NOT NULL,
                end TIMESTAMP NOT NULL,
                FOREIGN KEY (device_id) REFERENCES devices (id) ON DELETE CASCADE
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS devices_settings;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["devices"]
    }
}
