use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Table;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Device {
    pub id: i32,
    pub region_id: i32,
    pub name: String,
    pub device_type: i32,
    pub location: Value,
    pub status: Value,
}

#[derive(Clone)]
pub struct DeviceTable;

impl Table for DeviceTable {
    fn name(&self) -> &'static str {
        "devices"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS devices (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                region_id INTEGER NOT NULL,
                name VARCHAR(255) NOT NULL UNIQUE,
                device_type INTEGER NOT NULL,
                location JSON,
                status JSON,
                FOREIGN KEY (region_id) REFERENCES regions (id) ON DELETE CASCADE
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS devices;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["regions"]
    }
}
