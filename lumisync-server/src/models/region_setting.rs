use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::Table;

#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RegionSetting {
    pub id: i32,
    pub region_id: i32,
    pub min_light: i32,
    pub max_light: i32,
    pub min_temperature: f32,
    pub max_temperature: f32,
    pub start: OffsetDateTime,
    pub end: OffsetDateTime,
}

#[derive(Clone)]
pub struct RegionSettingTable;

impl Table for RegionSettingTable {
    fn name(&self) -> &'static str {
        "regions_settings"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS regions_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                region_id INTEGER NOT NULL,
                min_light INTEGER NOT NULL,
                max_light INTEGER NOT NULL,
                min_temperature REAL NOT NULL,
                max_temperature REAL NOT NULL,
                start TIMESTAMP NOT NULL,
                end TIMESTAMP NOT NULL,
                FOREIGN KEY (region_id) REFERENCES regions (id) ON DELETE CASCADE
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS regions_settings;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["regions"]
    }
}
