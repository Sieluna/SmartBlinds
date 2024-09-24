use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::Table;

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WindowRecord {
    pub id: i32,
    pub window_id: i32,
    /// State in a range of [-1, 1].
    pub state: f32,
    pub time: OffsetDateTime,
}

#[derive(Clone)]
pub struct WindowRecordTable;

impl Table for WindowRecordTable {
    fn name(&self) -> &'static str {
        "window_records"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS window_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                window_id INTEGER NOT NULL,
                state REAL NOT NULL,
                time TIMESTAMP NOT NULL,
                FOREIGN KEY (window_id) REFERENCES windows (id) ON DELETE CASCADE
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS window_records;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["windows"]
    }
}
