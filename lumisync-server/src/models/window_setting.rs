use serde::{Deserialize, Serialize};

use crate::models::Table;

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct WindowSetting {
    pub id: i32,
    pub window_id: i32,
    pub setting_id: i32,
}

pub struct WindowSettingTable;

impl Table for WindowSettingTable {
    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS windows_settings_link (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                window_id INTEGER NOT NULL,
                setting_id INTEGER NOT NULL,
                FOREIGN KEY (window_id) REFERENCES windows (id) ON DELETE CASCADE,
                FOREIGN KEY (setting_id) REFERENCES settings (id) ON DELETE CASCADE
            );
            "#
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS windows_settings_link;")
    }
}