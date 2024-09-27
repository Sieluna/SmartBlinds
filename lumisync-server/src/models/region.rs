use serde::{Deserialize, Serialize};

use super::Table;

#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Region {
    pub id: i32,
    pub group_id: i32,
    /// The name of the region.
    pub name: String,
    /// Light is the global lumen in room, average of sensors in region.
    pub light: i32,
    /// Temperature is the global temperature in room, average of sensors in region.
    pub temperature: f32,
    /// Humidity is the global humidity in room, average of sensors in region.
    pub humidity: f32,
    /// Whether the region is publicly accessible
    pub is_public: bool,
}

#[derive(Clone)]
pub struct RegionTable;

impl Table for RegionTable {
    fn name(&self) -> &'static str {
        "regions"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS regions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_id INTEGER NOT NULL,
                name VARCHAR(255) NOT NULL UNIQUE,
                light INTEGER NOT NULL,
                temperature REAL NOT NULL,
                humidity REAL NOT NULL,
                is_public BOOLEAN NOT NULL DEFAULT TRUE,
                FOREIGN KEY (group_id) REFERENCES groups (id) ON DELETE CASCADE
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS regions;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["groups"]
    }
}
