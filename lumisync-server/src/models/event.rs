use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use super::Table;

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Event {
    pub id: i32,
    pub event_type: String,
    pub payload: Value,
    pub time: OffsetDateTime,
}

#[derive(Clone)]
pub struct EventTable;

impl Table for EventTable {
    fn name(&self) -> &'static str {
        "events"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type VARCHAR(255) NOT NULL,
                payload JSON NOT NULL DEFAULT '{}',
                time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS events;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }
}
